use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[cfg(feature = "ai")]
use openagent_terminal::ai::agents::{
    AgentContext, AgentRequest, AgentRequestType,
    natural_language::NaturalLanguageAgent,
    workflow_orchestrator::WorkflowOrchestrator,
    communication_hub::AgentCommunicationHub,
};

fn setup_test_context() -> AgentContext {
    AgentContext {
        project_root: Some("/tmp/test-project".to_string()),
        current_directory: "/tmp/test-project/src".to_string(),
        current_branch: Some("main".to_string()),
        open_files: vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        recent_commands: vec![
            "cargo build".to_string(),
            "git status".to_string(),
            "ls -la".to_string(),
        ],
        environment_vars: {
            let mut env = HashMap::new();
            env.insert("RUST_LOG".to_string(), "debug".to_string());
            env.insert("CARGO_HOME".to_string(), "/home/user/.cargo".to_string());
            env
        },
        user_preferences: {
            let mut prefs = HashMap::new();
            prefs.insert("editor".to_string(), "vim".to_string());
            prefs.insert("shell".to_string(), "zsh".to_string());
            prefs
        },
    }
}

#[cfg(feature = "ai")]
fn bench_natural_language_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("natural_language_agent");
    
    // Set sample size and measurement time
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));
    
    let test_inputs = vec![
        "create a new rust function",
        "check security",
        "git commit changes",
        "analyze performance",
        "run tests",
        "help refactor code",
        "list files",
        "generate tests",
    ];
    
    for (i, input) in test_inputs.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("basic_processing", i),
            input,
            |b, input| {
                let agent = NaturalLanguageAgent::new();
                let context = setup_test_context();
                
                b.iter(|| {
                    // Simulate basic processing
                    let result = format!("Processed: {}", input);
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

#[cfg(feature = "ai")]
fn bench_workflow_orchestration(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_orchestration");
    
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));
    
    // Bench workflow setup simulation
    group.bench_function("workflow_setup", |b| {
        b.iter(|| {
            let agent = WorkflowOrchestrator::new();
            
            // Simulate workflow creation
            let workflow_data: HashMap<String, String> = HashMap::new();
            black_box((agent, workflow_data))
        });
    });
    
    // Bench workflow state management simulation
    group.bench_function("workflow_state_management", |b| {
        b.iter(|| {
            let mut state = HashMap::new();
            
            // Simulate state updates
            for i in 0..10 {
                state.insert(format!("step_{}", i), format!("status_{}", i % 3));
            }
            
            black_box(state)
        });
    });
    
    group.finish();
}

#[cfg(feature = "ai")]
fn bench_agent_communication(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_communication");
    
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));
    
    // Bench communication hub setup
    group.bench_function("communication_hub_setup", |b| {
        b.iter(|| {
            let hub = AgentCommunicationHub::new();
            black_box(hub)
        });
    });
    
    // Bench agent request simulation
    group.bench_function("request_simulation", |b| {
        b.iter(|| {
            let context = setup_test_context();
            let request = AgentRequest {
                id: Uuid::new_v4(),
                request_type: AgentRequestType::Custom("test".to_string()),
                payload: serde_json::json!({"test": true}),
                context,
                metadata: HashMap::new(),
            };
            
            black_box(request)
        });
    });
    
    group.finish();
}

#[cfg(feature = "ai")]
fn bench_agent_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_initialization");
    
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));
    
    // Bench agent creation simulation
    group.bench_function("agent_creation", |b| {
        b.iter(|| {
            let agent = NaturalLanguageAgent::new();
            black_box(agent)
        });
    });
    
    // Bench context setup
    group.bench_function("context_setup", |b| {
        b.iter(|| {
            let context = setup_test_context();
            black_box(context)
        });
    });
    
    group.finish();
}

#[cfg(feature = "ai")]
fn bench_intent_classification(c: &mut Criterion) {
    let mut group = c.benchmark_group("intent_classification");
    
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));
    
    let test_cases = vec![
        ("generate code", "code_generation"),
        ("check security", "security_analysis"),
        ("git commit", "git_operations"),
        ("analyze file performance", "file_operations"),
        ("create function", "code_generation"),
    ];
    
    for (input, expected_intent) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("classify_intent", expected_intent),
            &input,
            |b, input| {
                let context = setup_test_context();
                
                b.iter(|| {
                    // Simulate intent classification
                    let result = match input.contains("code") {
                        true => "code_generation",
                        false => "general",
                    };
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

#[cfg(feature = "ai")]
criterion_group!(
    ai_benches,
    bench_natural_language_processing,
    bench_workflow_orchestration,
    bench_agent_communication,
    bench_agent_initialization,
    bench_intent_classification
);

#[cfg(not(feature = "ai"))]
criterion_group!(ai_benches,);

criterion_main!(ai_benches);