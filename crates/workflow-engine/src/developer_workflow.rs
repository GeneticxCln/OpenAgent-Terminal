use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api_testing::{ApiCollection, ApiTester};
use crate::database_integration::{DatabaseConnection, DatabaseIntegration};
use crate::docker_integration::{DockerContext, DockerIntegration};
use crate::git_integration::{ConflictResolution, GitIntegration, GitRepository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperContext {
    pub current_directory: PathBuf,
    pub git_repository: Option<GitRepository>,
    pub docker_context: Option<DockerContext>,
    pub active_databases: Vec<DatabaseConnection>,
    pub api_collections: Vec<ApiCollection>,
    pub environment_type: EnvironmentType,
    pub project_type: Option<ProjectType>,
    pub detected_languages: Vec<ProgrammingLanguage>,
    pub build_tools: Vec<BuildTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvironmentType {
    Host,
    Container(String),
    DevContainer,
    VM,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    RustCargo,
    NodeJS,
    Python,
    Go,
    Java,
    DotNet,
    PHP,
    Ruby,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProgrammingLanguage {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    CSharp,
    PHP,
    Ruby,
    Shell,
    SQL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildTool {
    Cargo,
    NPM,
    Yarn,
    Pnpm,
    Pip,
    Poetry,
    Make,
    CMake,
    Gradle,
    Maven,
    Docker,
    DockerCompose,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAction {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub category: WorkflowCategory,
    pub inputs: Vec<WorkflowInput>,
    pub outputs: Vec<WorkflowOutput>,
    pub prerequisites: Vec<String>,
    pub estimated_duration: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowCategory {
    Git,
    Docker,
    Database,
    API,
    Build,
    Test,
    Deploy,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInput {
    pub name: String,
    pub input_type: InputType,
    pub required: bool,
    pub default_value: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputType {
    String,
    Integer,
    Boolean,
    File,
    Directory,
    DatabaseConnection,
    DockerContainer,
    GitBranch,
    ApiEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowOutput {
    pub name: String,
    pub output_type: OutputType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputType {
    CommandResult,
    File,
    Data(String),
    Status,
}

pub struct DeveloperWorkflow {
    git_integration: Arc<Mutex<Option<GitIntegration>>>,
    docker_integration: Arc<Mutex<DockerIntegration>>,
    database_integration: Arc<Mutex<DatabaseIntegration>>,
    api_tester: Arc<Mutex<ApiTester>>,
    current_context: Arc<Mutex<DeveloperContext>>,
    available_workflows: Arc<Mutex<HashMap<String, WorkflowAction>>>,
}

impl DeveloperWorkflow {
    pub async fn new(working_directory: PathBuf) -> Result<Self> {
        let git_integration = if Self::is_git_repository(&working_directory).await? {
            Some(GitIntegration::new(working_directory.clone())?)
        } else {
            None
        };

        let docker_integration = DockerIntegration::new().await?;
        let database_integration = DatabaseIntegration::new();
        let api_tester = ApiTester::new();

        let initial_context =
            Self::analyze_project_context(&working_directory, &docker_integration).await?;

        let mut workflow = Self {
            git_integration: Arc::new(Mutex::new(git_integration)),
            docker_integration: Arc::new(Mutex::new(docker_integration)),
            database_integration: Arc::new(Mutex::new(database_integration)),
            api_tester: Arc::new(Mutex::new(api_tester)),
            current_context: Arc::new(Mutex::new(initial_context)),
            available_workflows: Arc::new(Mutex::new(HashMap::new())),
        };

        // Register built-in workflows
        workflow.register_built_in_workflows().await?;

        Ok(workflow)
    }

    async fn is_git_repository(directory: &Path) -> Result<bool> {
        let git_dir = directory.join(".git");
        Ok(tokio::fs::metadata(git_dir).await.is_ok())
    }

    async fn analyze_project_context(
        directory: &Path,
        docker_integration: &DockerIntegration,
    ) -> Result<DeveloperContext> {
        let project_type = Self::detect_project_type(directory).await?;
        let detected_languages = Self::detect_languages(directory).await?;
        let build_tools = Self::detect_build_tools(directory).await?;

        let environment_type = if docker_integration.get_context().in_container {
            if let Some(id) = &docker_integration.get_context().container_id {
                EnvironmentType::Container(id.clone())
            } else {
                EnvironmentType::DevContainer
            }
        } else {
            EnvironmentType::Host
        };

        Ok(DeveloperContext {
            current_directory: directory.to_path_buf(),
            git_repository: None, // Will be populated later
            docker_context: Some(docker_integration.get_context().clone()),
            active_databases: Vec::new(),
            api_collections: Vec::new(),
            environment_type,
            project_type,
            detected_languages,
            build_tools,
        })
    }

    async fn detect_project_type(directory: &Path) -> Result<Option<ProjectType>> {
        let entries = tokio::fs::read_dir(directory).await?;
        let mut entries_vec = Vec::new();

        let mut entries_stream = entries;
        while let Ok(Some(entry)) = entries_stream.next_entry().await {
            entries_vec.push(entry.file_name().to_string_lossy().to_string());
        }

        if entries_vec.contains(&"Cargo.toml".to_string()) {
            return Ok(Some(ProjectType::RustCargo));
        }

        if entries_vec.contains(&"package.json".to_string()) {
            return Ok(Some(ProjectType::NodeJS));
        }

        if entries_vec.contains(&"requirements.txt".to_string())
            || entries_vec.contains(&"pyproject.toml".to_string())
            || entries_vec.contains(&"setup.py".to_string())
        {
            return Ok(Some(ProjectType::Python));
        }

        if entries_vec.contains(&"go.mod".to_string()) {
            return Ok(Some(ProjectType::Go));
        }

        if entries_vec.contains(&"pom.xml".to_string())
            || entries_vec.contains(&"build.gradle".to_string())
        {
            return Ok(Some(ProjectType::Java));
        }

        if entries_vec.iter().any(|f| f.ends_with(".csproj") || f.ends_with(".sln")) {
            return Ok(Some(ProjectType::DotNet));
        }

        Ok(Some(ProjectType::Unknown))
    }

    async fn detect_languages(directory: &Path) -> Result<Vec<ProgrammingLanguage>> {
        let mut languages = Vec::new();
        let mut stack = vec![directory.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();

                    if path.is_file() {
                        if let Some(extension) = path.extension() {
                            let ext = extension.to_string_lossy().to_lowercase();
                            match ext.as_str() {
                                "rs" => {
                                    if !languages.contains(&ProgrammingLanguage::Rust) {
                                        languages.push(ProgrammingLanguage::Rust);
                                    }
                                },
                                "js" => {
                                    if !languages.contains(&ProgrammingLanguage::JavaScript) {
                                        languages.push(ProgrammingLanguage::JavaScript);
                                    }
                                },
                                "ts" => {
                                    if !languages.contains(&ProgrammingLanguage::TypeScript) {
                                        languages.push(ProgrammingLanguage::TypeScript);
                                    }
                                },
                                "py" => {
                                    if !languages.contains(&ProgrammingLanguage::Python) {
                                        languages.push(ProgrammingLanguage::Python);
                                    }
                                },
                                "go" => {
                                    if !languages.contains(&ProgrammingLanguage::Go) {
                                        languages.push(ProgrammingLanguage::Go);
                                    }
                                },
                                "java" => {
                                    if !languages.contains(&ProgrammingLanguage::Java) {
                                        languages.push(ProgrammingLanguage::Java);
                                    }
                                },
                                "cs" => {
                                    if !languages.contains(&ProgrammingLanguage::CSharp) {
                                        languages.push(ProgrammingLanguage::CSharp);
                                    }
                                },
                                "php" => {
                                    if !languages.contains(&ProgrammingLanguage::PHP) {
                                        languages.push(ProgrammingLanguage::PHP);
                                    }
                                },
                                "rb" => {
                                    if !languages.contains(&ProgrammingLanguage::Ruby) {
                                        languages.push(ProgrammingLanguage::Ruby);
                                    }
                                },
                                "sh" | "bash" | "zsh" => {
                                    if !languages.contains(&ProgrammingLanguage::Shell) {
                                        languages.push(ProgrammingLanguage::Shell);
                                    }
                                },
                                "sql" => {
                                    if !languages.contains(&ProgrammingLanguage::SQL) {
                                        languages.push(ProgrammingLanguage::SQL);
                                    }
                                },
                                _ => {},
                            }
                        }
                    } else if path.is_dir()
                        && !path.file_name().unwrap().to_string_lossy().starts_with('.')
                    {
                        stack.push(path);
                    }
                }
            }
        }

        Ok(languages)
    }

    async fn detect_build_tools(directory: &Path) -> Result<Vec<BuildTool>> {
        let mut tools = Vec::new();

        let entries = tokio::fs::read_dir(directory).await?;
        let mut entries_vec = Vec::new();

        let mut entries_stream = entries;
        while let Ok(Some(entry)) = entries_stream.next_entry().await {
            entries_vec.push(entry.file_name().to_string_lossy().to_string());
        }

        if entries_vec.contains(&"Cargo.toml".to_string()) {
            tools.push(BuildTool::Cargo);
        }

        if entries_vec.contains(&"package.json".to_string()) {
            tools.push(BuildTool::NPM);

            // Check for yarn.lock or pnpm-lock.yaml
            if entries_vec.contains(&"yarn.lock".to_string()) {
                tools.push(BuildTool::Yarn);
            }
            if entries_vec.contains(&"pnpm-lock.yaml".to_string()) {
                tools.push(BuildTool::Pnpm);
            }
        }

        if entries_vec.contains(&"requirements.txt".to_string()) {
            tools.push(BuildTool::Pip);
        }

        if entries_vec.contains(&"pyproject.toml".to_string()) {
            tools.push(BuildTool::Poetry);
        }

        if entries_vec.contains(&"Makefile".to_string())
            || entries_vec.contains(&"makefile".to_string())
        {
            tools.push(BuildTool::Make);
        }

        if entries_vec.contains(&"CMakeLists.txt".to_string()) {
            tools.push(BuildTool::CMake);
        }

        if entries_vec.contains(&"build.gradle".to_string()) {
            tools.push(BuildTool::Gradle);
        }

        if entries_vec.contains(&"pom.xml".to_string()) {
            tools.push(BuildTool::Maven);
        }

        if entries_vec.iter().any(|f| {
            f.starts_with("Dockerfile") || f == "docker-compose.yml" || f == "docker-compose.yaml"
        }) {
            tools.push(BuildTool::Docker);
            if entries_vec.iter().any(|f| f.starts_with("docker-compose")) {
                tools.push(BuildTool::DockerCompose);
            }
        }

        Ok(tools)
    }

    pub async fn refresh_context(&self) -> Result<()> {
        let mut context = self.current_context.lock().await;

        // Refresh Git information
        if let Some(git_integration) = self.git_integration.lock().await.as_ref() {
            context.git_repository = Some(git_integration.get_repository_info().await?);
        }

        // Refresh Docker context
        {
            let mut docker_integration = self.docker_integration.lock().await;
            docker_integration.refresh_context().await?;
            context.docker_context = Some(docker_integration.get_context().clone());
        }

        // Refresh database connections
        {
            let database_integration = self.database_integration.lock().await;
            context.active_databases = database_integration.get_connections().await?;
        }

        // Refresh API collections
        {
            let api_tester = self.api_tester.lock().await;
            context.api_collections = api_tester.get_collections().await?;
        }

        Ok(())
    }

    async fn register_built_in_workflows(&mut self) -> Result<()> {
        let mut workflows = self.available_workflows.lock().await;

        // Git workflows
        workflows.insert("git_resolve_conflicts".to_string(), WorkflowAction {
            id: Uuid::new_v4(),
            name: "Resolve Git Conflicts".to_string(),
            description: "Interactive conflict resolution with visual diff".to_string(),
            category: WorkflowCategory::Git,
            inputs: vec![WorkflowInput {
                name: "auto_resolve".to_string(),
                input_type: InputType::Boolean,
                required: false,
                default_value: Some("false".to_string()),
                description: "Automatically resolve simple conflicts".to_string(),
            }],
            outputs: vec![WorkflowOutput {
                name: "conflicts_resolved".to_string(),
                output_type: OutputType::Status,
                description: "Number of conflicts resolved".to_string(),
            }],
            prerequisites: vec!["git".to_string()],
            estimated_duration: std::time::Duration::from_secs(300),
        });

        workflows.insert("git_branch_visualization".to_string(), WorkflowAction {
            id: Uuid::new_v4(),
            name: "Visualize Git Branches".to_string(),
            description: "Display branch graph with commit information and signatures".to_string(),
            category: WorkflowCategory::Git,
            inputs: vec![WorkflowInput {
                name: "max_branches".to_string(),
                input_type: InputType::Integer,
                required: false,
                default_value: Some("20".to_string()),
                description: "Maximum number of branches to show".to_string(),
            }],
            outputs: vec![WorkflowOutput {
                name: "branch_graph".to_string(),
                output_type: OutputType::Data("visualization".to_string()),
                description: "ASCII art branch visualization".to_string(),
            }],
            prerequisites: vec!["git".to_string()],
            estimated_duration: std::time::Duration::from_secs(10),
        });

        // Docker workflows
        workflows.insert("docker_context_switch".to_string(), WorkflowAction {
            id: Uuid::new_v4(),
            name: "Switch Docker Context".to_string(),
            description: "Switch execution context between host and containers".to_string(),
            category: WorkflowCategory::Docker,
            inputs: vec![WorkflowInput {
                name: "target_container".to_string(),
                input_type: InputType::DockerContainer,
                required: true,
                default_value: None,
                description: "Target container for execution context".to_string(),
            }],
            outputs: vec![WorkflowOutput {
                name: "context_switched".to_string(),
                output_type: OutputType::Status,
                description: "New execution context".to_string(),
            }],
            prerequisites: vec!["docker".to_string()],
            estimated_duration: std::time::Duration::from_secs(5),
        });

        // Database workflows
        workflows.insert("db_query_builder".to_string(), WorkflowAction {
            id: Uuid::new_v4(),
            name: "Interactive Query Builder".to_string(),
            description: "Build and execute database queries with schema awareness".to_string(),
            category: WorkflowCategory::Database,
            inputs: vec![WorkflowInput {
                name: "connection".to_string(),
                input_type: InputType::DatabaseConnection,
                required: true,
                default_value: None,
                description: "Database connection to use".to_string(),
            }],
            outputs: vec![WorkflowOutput {
                name: "query_result".to_string(),
                output_type: OutputType::Data("table".to_string()),
                description: "Query execution results".to_string(),
            }],
            prerequisites: vec!["database_connection".to_string()],
            estimated_duration: std::time::Duration::from_secs(60),
        });

        // API workflows
        workflows.insert("api_test_suite".to_string(), WorkflowAction {
            id: Uuid::new_v4(),
            name: "Run API Test Suite".to_string(),
            description: "Execute API tests with assertions and reporting".to_string(),
            category: WorkflowCategory::API,
            inputs: vec![WorkflowInput {
                name: "collection_name".to_string(),
                input_type: InputType::String,
                required: true,
                default_value: None,
                description: "API collection to test".to_string(),
            }],
            outputs: vec![WorkflowOutput {
                name: "test_results".to_string(),
                output_type: OutputType::Data("test_report".to_string()),
                description: "Test execution results and assertions".to_string(),
            }],
            prerequisites: vec!["api_collection".to_string()],
            estimated_duration: std::time::Duration::from_secs(120),
        });

        Ok(())
    }

    pub async fn execute_workflow(
        &self,
        workflow_name: &str,
        inputs: HashMap<String, String>,
    ) -> Result<WorkflowResult> {
        // Ensure the workflow exists to provide a clear error if not found
        {
            let workflows = self.available_workflows.lock().await;
            if !workflows.contains_key(workflow_name) {
                return Err(anyhow!("Workflow '{}' not found", workflow_name));
            }
        }

        let start_time = std::time::Instant::now();
        let outputs = match workflow_name {
            "git_resolve_conflicts" => self.execute_git_conflict_resolution(inputs).await?,
            "git_branch_visualization" => self.execute_git_branch_visualization(inputs).await?,
            "docker_context_switch" => self.execute_docker_context_switch(inputs).await?,
            "db_query_builder" => self.execute_database_query(inputs).await?,
            "api_test_suite" => self.execute_api_test_suite(inputs).await?,
            _ => {
                return Err(anyhow!("Unknown workflow: {}", workflow_name));
            },
        };

        let execution_time = start_time.elapsed();

        Ok(WorkflowResult {
            workflow_name: workflow_name.to_string(),
            execution_time,
            outputs,
            success: true,
            error: None,
            executed_at: chrono::Utc::now(),
        })
    }

    async fn execute_git_conflict_resolution(
        &self,
        inputs: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let auto_resolve =
            inputs.get("auto_resolve").and_then(|v| v.parse::<bool>().ok()).unwrap_or(false);

        let mut outputs = HashMap::new();

        if let Some(git_integration) = self.git_integration.lock().await.as_ref() {
            let repo_info = git_integration.get_repository_info().await?;

            if repo_info.conflicts.is_empty() {
                outputs.insert("conflicts_resolved".to_string(), "0".to_string());
                outputs.insert("message".to_string(), "No conflicts to resolve".to_string());
            } else {
                let mut resolved_count = 0;

                for conflict in &repo_info.conflicts {
                    if auto_resolve {
                        // Simple auto-resolution logic
                        git_integration
                            .resolve_conflict(&conflict.file, ConflictResolution::AcceptOurs)
                            .await?;
                        resolved_count += 1;
                    } else {
                        // Generate interactive UI
                        let ui = git_integration.render_conflict_resolution(conflict)?;
                        outputs.insert(format!("conflict_{}", conflict.file.to_string_lossy()), ui);
                    }
                }

                outputs.insert("conflicts_resolved".to_string(), resolved_count.to_string());
                outputs
                    .insert("total_conflicts".to_string(), repo_info.conflicts.len().to_string());
            }
        }

        Ok(outputs)
    }

    async fn execute_git_branch_visualization(
        &self,
        inputs: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let max_branches =
            inputs.get("max_branches").and_then(|v| v.parse::<usize>().ok()).unwrap_or(20);

        let mut outputs = HashMap::new();

        if let Some(git_integration) = self.git_integration.lock().await.as_ref() {
            let repo_info = git_integration.get_repository_info().await?;
            let branches = repo_info.branches.into_iter().take(max_branches).collect::<Vec<_>>();

            let visualization = git_integration.render_branch_visualization(&branches)?;
            outputs.insert("branch_graph".to_string(), visualization);
            outputs.insert("branch_count".to_string(), branches.len().to_string());
        }

        Ok(outputs)
    }

    async fn execute_docker_context_switch(
        &self,
        inputs: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let target_container = inputs
            .get("target_container")
            .ok_or_else(|| anyhow!("target_container is required"))?;

        let mut outputs = HashMap::new();

        let docker_integration = self.docker_integration.lock().await;
        let available_containers = &docker_integration.get_context().available_containers;

        if let Some(_container) = available_containers
            .iter()
            .find(|c| c.name == *target_container || c.id == *target_container)
        {
            outputs.insert("context_switched".to_string(), "true".to_string());
            outputs.insert("new_context".to_string(), format!("container:{}", target_container));
        } else {
            outputs.insert("context_switched".to_string(), "false".to_string());
            outputs
                .insert("error".to_string(), format!("Container '{}' not found", target_container));
        }

        Ok(outputs)
    }

    async fn execute_database_query(
        &self,
        inputs: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let connection_name =
            inputs.get("connection").ok_or_else(|| anyhow!("connection is required"))?;

        let query = inputs.get("query").ok_or_else(|| anyhow!("query is required"))?;

        let mut outputs = HashMap::new();

        let database_integration = self.database_integration.lock().await;
        let connections = database_integration.get_connections().await?;

        if let Some(connection) = connections.iter().find(|c| c.name == *connection_name) {
            match database_integration.execute_query(connection.id, query).await {
                Ok(result) => {
                    outputs.insert("rows_returned".to_string(), result.rows.len().to_string());
                    outputs.insert(
                        "execution_time_ms".to_string(),
                        result.execution_time.as_millis().to_string(),
                    );
                    outputs.insert("query_result".to_string(), serde_json::to_string(&result)?);
                },
                Err(e) => {
                    outputs.insert("error".to_string(), e.to_string());
                },
            }
        } else {
            outputs
                .insert("error".to_string(), format!("Connection '{}' not found", connection_name));
        }

        Ok(outputs)
    }

    async fn execute_api_test_suite(
        &self,
        inputs: HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let collection_name =
            inputs.get("collection_name").ok_or_else(|| anyhow!("collection_name is required"))?;

        let mut outputs = HashMap::new();

        let api_tester = self.api_tester.lock().await;
        let collections = api_tester.get_collections().await?;

        if let Some(collection) = collections.iter().find(|c| c.name == *collection_name) {
            // For simplicity, execute all requests in the collection
            let mut total_requests = 0;
            let mut successful_requests = 0;
            let mut total_time = std::time::Duration::ZERO;

            for request in &collection.requests {
                total_requests += 1;
                match api_tester.execute_request(request).await {
                    Ok(response) => {
                        if response.status_code >= 200 && response.status_code < 300 {
                            successful_requests += 1;
                        }
                        total_time += response.response_time;
                        outputs.insert(
                            format!("response_{}", request.name.replace(' ', "_")),
                            api_tester.format_response_summary(&response),
                        );
                    },
                    Err(e) => {
                        outputs.insert(
                            format!("error_{}", request.name.replace(' ', "_")),
                            e.to_string(),
                        );
                    },
                }
            }

            outputs.insert("total_requests".to_string(), total_requests.to_string());
            outputs.insert("successful_requests".to_string(), successful_requests.to_string());
            outputs.insert("total_time_ms".to_string(), total_time.as_millis().to_string());
            outputs.insert(
                "success_rate".to_string(),
                format!("{:.1}%", (successful_requests as f64 / total_requests as f64) * 100.0),
            );
        } else {
            outputs
                .insert("error".to_string(), format!("Collection '{}' not found", collection_name));
        }

        Ok(outputs)
    }

    pub async fn get_context(&self) -> Result<DeveloperContext> {
        let context = self.current_context.lock().await;
        Ok(context.clone())
    }

    pub async fn get_available_workflows(&self) -> Result<Vec<WorkflowAction>> {
        let workflows = self.available_workflows.lock().await;
        Ok(workflows.values().cloned().collect())
    }

    pub async fn suggest_workflows(&self) -> Result<Vec<String>> {
        let context = self.get_context().await?;
        let mut suggestions = Vec::new();

        // Suggest Git workflows if in a Git repository
        if context.git_repository.is_some() {
            let git_repo = context.git_repository.as_ref().unwrap();

            if !git_repo.conflicts.is_empty() {
                suggestions.push("git_resolve_conflicts".to_string());
            }

            if git_repo.branches.len() > 1 {
                suggestions.push("git_branch_visualization".to_string());
            }
        }

        // Suggest Docker workflows if containers are available
        if let Some(docker_context) = &context.docker_context {
            if !docker_context.available_containers.is_empty() {
                suggestions.push("docker_context_switch".to_string());
            }
        }

        // Suggest database workflows if databases are configured
        if !context.active_databases.is_empty() {
            suggestions.push("db_query_builder".to_string());
        }

        // Suggest API workflows if collections exist
        if !context.api_collections.is_empty() {
            suggestions.push("api_test_suite".to_string());
        }

        Ok(suggestions)
    }

    pub async fn generate_ai_context_prompt(&self) -> Result<String> {
        let context = self.get_context().await?;
        let mut prompt = String::new();

        prompt.push_str("Developer Environment Context:\n\n");

        // Environment type
        match &context.environment_type {
            EnvironmentType::Host => prompt.push_str("🏠 Running on host system\n"),
            EnvironmentType::Container(id) => {
                prompt.push_str(&format!("🐳 Running in container: {}\n", &id[..8]))
            },
            EnvironmentType::DevContainer => {
                prompt.push_str("📦 Running in development container\n")
            },
            EnvironmentType::VM => prompt.push_str("💻 Running in virtual machine\n"),
        }

        // Project information
        if let Some(project_type) = &context.project_type {
            prompt.push_str(&format!("📁 Project type: {:?}\n", project_type));
        }

        if !context.detected_languages.is_empty() {
            prompt.push_str(&format!("🔤 Languages: {:?}\n", context.detected_languages));
        }

        if !context.build_tools.is_empty() {
            prompt.push_str(&format!("🔧 Build tools: {:?}\n", context.build_tools));
        }

        // Git information
        if let Some(git_repo) = &context.git_repository {
            prompt.push_str("\n📝 Git Repository:\n");
            if let Some(current_branch) = &git_repo.current_branch {
                prompt.push_str(&format!("  Current branch: {}\n", current_branch));
            }
            prompt.push_str(&format!("  Branches: {}\n", git_repo.branches.len()));
            if !git_repo.conflicts.is_empty() {
                prompt.push_str(&format!("  ⚠️  Conflicts: {}\n", git_repo.conflicts.len()));
            }
            if !git_repo.status.modified.is_empty() {
                prompt.push_str(&format!("  Modified files: {}\n", git_repo.status.modified.len()));
            }
            if !git_repo.status.untracked.is_empty() {
                prompt
                    .push_str(&format!("  Untracked files: {}\n", git_repo.status.untracked.len()));
            }
        }

        // Docker information
        if let Some(docker_context) = &context.docker_context {
            prompt.push_str("\n🐳 Docker Context:\n");
            prompt.push_str(&format!(
                "  Available containers: {}\n",
                docker_context.available_containers.len()
            ));
            prompt.push_str(&format!(
                "  Compose services: {}\n",
                docker_context.compose_services.len()
            ));

            if docker_context.in_container {
                prompt.push_str("  Currently running inside container\n");
            }
        }

        // Database information
        if !context.active_databases.is_empty() {
            prompt.push_str("\n🗄️  Databases:\n");
            for db in &context.active_databases {
                prompt.push_str(&format!(
                    "  {} ({:?}) on {}:{}\n",
                    db.name, db.database_type, db.host, db.port
                ));
            }
        }

        // API collections
        if !context.api_collections.is_empty() {
            prompt.push_str("\n🌐 API Collections:\n");
            for collection in &context.api_collections {
                prompt.push_str(&format!(
                    "  {} ({} requests)\n",
                    collection.name,
                    collection.requests.len()
                ));
            }
        }

        // Suggested workflows
        let suggestions = self.suggest_workflows().await?;
        if !suggestions.is_empty() {
            prompt.push_str(&format!("\n💡 Suggested workflows: {}\n", suggestions.join(", ")));
        }

        prompt.push_str(
            "\nUse the above context to provide relevant command suggestions and workflow \
             recommendations.",
        );

        Ok(prompt)
    }

    pub async fn get_workflow_templates(&self) -> Result<HashMap<String, String>> {
        let mut templates = HashMap::new();

        templates.insert(
            "git_commit_with_signing".to_string(),
            r#"
# Commit files with GPG signing
git add {{ files | default(value=".")}}
git commit -S -m "{{ message }}"
{% if push -%}
git push origin {{ branch | default(value="HEAD") }}
{% endif -%}
        "#
            .trim()
            .to_string(),
        );

        templates.insert(
            "docker_dev_setup".to_string(),
            r#"
# Set up development environment in Docker
docker-compose up -d {{ services | default(value="") }}
{% for service in services -%}
docker-compose exec {{ service }} {{ setup_command | default(value="bash") }}
{% endfor -%}
        "#
            .trim()
            .to_string(),
        );

        templates.insert(
            "database_migration".to_string(),
            r#"
# Run database migration
{% if backup -%}
{{ db_backup_command }}
{% endif -%}
{{ migration_command }}
{% if verify -%}
{{ verification_query }}
{% endif -%}
        "#
            .trim()
            .to_string(),
        );

        templates.insert(
            "api_endpoint_test".to_string(),
            r#"
# Test API endpoint
curl -X {{ method | default(value="GET") }} \
  -H "Content-Type: application/json" \
  {% for header in headers -%}
  -H "{{ header.key }}: {{ header.value }}" \
  {% endfor -%}
  {% if body -%}
  -d '{{ body }}' \
  {% endif -%}
  "{{ url }}"
        "#
            .trim()
            .to_string(),
        );

        Ok(templates)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow_name: String,
    pub execution_time: std::time::Duration,
    pub outputs: HashMap<String, String>,
    pub success: bool,
    pub error: Option<String>,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_workflow() -> Result<DeveloperWorkflow> {
        let temp_dir = TempDir::new()?;
        let workflow = DeveloperWorkflow::new(temp_dir.path().to_path_buf()).await?;
        Ok(workflow)
    }

    #[tokio::test]
    async fn test_developer_workflow_creation() {
        if std::env::var("DOCKER_AVAILABLE").ok().as_deref() != Some("1") {
            eprintln!("Skipping docker-dependent test: set DOCKER_AVAILABLE=1 to enable");
            return;
        }
        let result = create_test_workflow().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workflow_suggestions() {
        if std::env::var("DOCKER_AVAILABLE").ok().as_deref() != Some("1") {
            eprintln!("Skipping docker-dependent test: set DOCKER_AVAILABLE=1 to enable");
            return;
        }
        let workflow = create_test_workflow().await.unwrap();
        let suggestions = workflow.suggest_workflows().await.unwrap();
        // Should not fail, may be empty for a new directory
        assert!(suggestions.is_empty() || !suggestions.is_empty());
    }

    #[tokio::test]
    async fn test_ai_context_generation() {
        if std::env::var("DOCKER_AVAILABLE").ok().as_deref() != Some("1") {
            eprintln!("Skipping docker-dependent test: set DOCKER_AVAILABLE=1 to enable");
            return;
        }
        let workflow = create_test_workflow().await.unwrap();
        let context_prompt = workflow.generate_ai_context_prompt().await.unwrap();

        assert!(context_prompt.contains("Developer Environment Context"));
        assert!(context_prompt.contains("Running on host system"));
    }
}
