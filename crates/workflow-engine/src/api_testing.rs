use anyhow::{anyhow, Result};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub id: Uuid,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub body: Option<RequestBody>,
    pub auth: Option<Authentication>,
    pub timeout: Duration,
    pub follow_redirects: bool,
    pub verify_ssl: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub request_id: Uuid,
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: ResponseBody,
    pub response_time: Duration,
    pub size_bytes: usize,
    pub http_version: String,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
    CONNECT,
    TRACE,
}

impl From<HttpMethod> for Method {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::GET => Method::GET,
            HttpMethod::POST => Method::POST,
            HttpMethod::PUT => Method::PUT,
            HttpMethod::DELETE => Method::DELETE,
            HttpMethod::PATCH => Method::PATCH,
            HttpMethod::HEAD => Method::HEAD,
            HttpMethod::OPTIONS => Method::OPTIONS,
            HttpMethod::CONNECT => Method::CONNECT,
            HttpMethod::TRACE => Method::TRACE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestBody {
    Text(String),
    JSON(serde_json::Value),
    FormData(HashMap<String, String>),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseBody {
    Text(String),
    JSON(serde_json::Value),
    Binary(Vec<u8>),
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Authentication {
    Bearer(String),
    Basic { username: String, password: String },
    ApiKey { key: String, value: String },
    OAuth2(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCollection {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub requests: Vec<ApiRequest>,
    pub variables: HashMap<String, String>,
    pub base_url: Option<String>,
    pub default_headers: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    pub id: Uuid,
    pub name: String,
    pub collection_id: Uuid,
    pub tests: Vec<ApiTest>,
    pub setup_requests: Vec<Uuid>,
    pub teardown_requests: Vec<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTest {
    pub id: Uuid,
    pub name: String,
    pub request_id: Uuid,
    pub assertions: Vec<Assertion>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Assertion {
    StatusCode(u16),
    StatusCodeRange { min: u16, max: u16 },
    HeaderExists(String),
    HeaderEquals { header: String, value: String },
    HeaderContains { header: String, substring: String },
    BodyContains(String),
    BodyEquals(String),
    BodyJsonPath { path: String, expected: serde_json::Value },
    ResponseTime { max_ms: u64 },
    ContentType(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_id: Uuid,
    pub passed: bool,
    pub assertion_results: Vec<AssertionResult>,
    pub response: ApiResponse,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub assertion: Assertion,
    pub passed: bool,
    pub message: String,
}

pub struct ApiTester {
    client: Client,
    collections: tokio::sync::Mutex<HashMap<Uuid, ApiCollection>>,
    response_history: tokio::sync::Mutex<Vec<ApiResponse>>,
    test_suites: tokio::sync::Mutex<HashMap<Uuid, TestSuite>>,
}

impl Default for ApiTester {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiTester {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("OpenAgent-Terminal/1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            collections: tokio::sync::Mutex::new(HashMap::new()),
            response_history: tokio::sync::Mutex::new(Vec::new()),
            test_suites: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn create_collection(&self, mut collection: ApiCollection) -> Result<Uuid> {
        collection.id = Uuid::new_v4();
        collection.created_at = chrono::Utc::now();
        collection.updated_at = collection.created_at;

        let id = collection.id;
        let mut collections = self.collections.lock().await;
        collections.insert(id, collection);

        Ok(id)
    }

    pub async fn add_request_to_collection(
        &self,
        collection_id: Uuid,
        mut request: ApiRequest,
    ) -> Result<Uuid> {
        request.id = Uuid::new_v4();
        request.created_at = chrono::Utc::now();
        request.updated_at = request.created_at;

        let mut collections = self.collections.lock().await;
        let collection =
            collections.get_mut(&collection_id).ok_or_else(|| anyhow!("Collection not found"))?;

        let request_id = request.id;
        collection.requests.push(request);
        collection.updated_at = chrono::Utc::now();

        Ok(request_id)
    }

    pub async fn execute_request(&self, request: &ApiRequest) -> Result<ApiResponse> {
        let start_time = Instant::now();

        // Build the request
        let mut req_builder = self.client.request(request.method.clone().into(), &request.url);

        // Add headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Add query parameters
        if !request.query_params.is_empty() {
            req_builder = req_builder.query(&request.query_params);
        }

        // Add authentication
        if let Some(auth) = &request.auth {
            req_builder = match auth {
                Authentication::Bearer(token) => req_builder.bearer_auth(token),
                Authentication::Basic { username, password } => {
                    req_builder.basic_auth(username, Some(password))
                }
                Authentication::ApiKey { key, value } => req_builder.header(key, value),
                Authentication::OAuth2(token) => req_builder.bearer_auth(token),
            };
        }

        // Add body
        if let Some(body) = &request.body {
            req_builder = match body {
                RequestBody::Text(text) => req_builder.body(text.clone()),
                RequestBody::JSON(json) => req_builder.json(json),
                RequestBody::FormData(form) => req_builder.form(form),
                RequestBody::Binary(data) => req_builder.body(data.clone()),
            };
        }

        // Set timeout
        req_builder = req_builder.timeout(request.timeout);

        // Execute the request
        let response = req_builder.send().await.map_err(|e| anyhow!("Request failed: {}", e))?;

        let response_time = start_time.elapsed();

        // Process response
        let status_code = response.status().as_u16();
        let status_text = response.status().canonical_reason().unwrap_or("Unknown").to_string();
        let http_version = format!("{:?}", response.version());

        let mut headers = HashMap::new();
        for (name, value) in response.headers() {
            headers.insert(name.to_string(), value.to_str().unwrap_or("").to_string());
        }

        let body_bytes =
            response.bytes().await.map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        let size_bytes = body_bytes.len();

        let response_body = self.parse_response_body(&headers, body_bytes.to_vec())?;

        let api_response = ApiResponse {
            request_id: request.id,
            status_code,
            status_text,
            headers,
            body: response_body,
            response_time,
            size_bytes,
            http_version,
            executed_at: chrono::Utc::now(),
        };

        // Store in history
        let mut history = self.response_history.lock().await;
        history.push(api_response.clone());

        // Keep only last 1000 responses
        if history.len() > 1000 {
            history.remove(0);
        }

        Ok(api_response)
    }

    fn parse_response_body(
        &self,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<ResponseBody> {
        if body.is_empty() {
            return Ok(ResponseBody::Empty);
        }

        let content_type = headers
            .get("content-type")
            .or_else(|| headers.get("Content-Type"))
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if content_type.contains("application/json") {
            match serde_json::from_slice(&body) {
                Ok(json_value) => Ok(ResponseBody::JSON(json_value)),
                Err(_) => {
                    // If JSON parsing fails, treat as text
                    match String::from_utf8(body.clone()) {
                        Ok(text) => Ok(ResponseBody::Text(text)),
                        Err(_) => Ok(ResponseBody::Binary(body)),
                    }
                }
            }
        } else if content_type.starts_with("text/") || content_type.contains("xml") {
            match String::from_utf8(body.clone()) {
                Ok(text) => Ok(ResponseBody::Text(text)),
                Err(_) => Ok(ResponseBody::Binary(body)),
            }
        } else {
            // Try to parse as text first
            match String::from_utf8(body.clone()) {
                Ok(text) => {
                    // Check if it looks like JSON even without proper content-type
                    if (text.trim().starts_with('{') && text.trim().ends_with('}'))
                        || (text.trim().starts_with('[') && text.trim().ends_with(']'))
                    {
                        if let Ok(json_value) = serde_json::from_str(&text) {
                            return Ok(ResponseBody::JSON(json_value));
                        }
                    }
                    Ok(ResponseBody::Text(text))
                }
                Err(_) => Ok(ResponseBody::Binary(body)),
            }
        }
    }

    pub async fn create_test_suite(&self, mut test_suite: TestSuite) -> Result<Uuid> {
        test_suite.id = Uuid::new_v4();
        test_suite.created_at = chrono::Utc::now();

        let id = test_suite.id;
        let mut test_suites = self.test_suites.lock().await;
        test_suites.insert(id, test_suite);

        Ok(id)
    }

    pub async fn run_test_suite(&self, test_suite_id: Uuid) -> Result<Vec<TestResult>> {
        let test_suite = {
            let test_suites = self.test_suites.lock().await;
            test_suites
                .get(&test_suite_id)
                .cloned()
                .ok_or_else(|| anyhow!("Test suite not found"))?
        };

        let collection = {
            let collections = self.collections.lock().await;
            collections
                .get(&test_suite.collection_id)
                .cloned()
                .ok_or_else(|| anyhow!("Collection not found"))?
        };

        let mut results = Vec::new();

        // Run setup requests
        for setup_request_id in &test_suite.setup_requests {
            if let Some(request) = collection.requests.iter().find(|r| r.id == *setup_request_id) {
                let _response = self.execute_request(request).await?;
            }
        }

        // Run test requests
        for test in &test_suite.tests {
            if !test.enabled {
                continue;
            }

            if let Some(request) = collection.requests.iter().find(|r| r.id == test.request_id) {
                let response = self.execute_request(request).await?;
                let test_result = self.evaluate_assertions(&test.assertions, &response).await?;

                let result = TestResult {
                    test_id: test.id,
                    passed: test_result.iter().all(|r| r.passed),
                    assertion_results: test_result,
                    response,
                    executed_at: chrono::Utc::now(),
                };

                results.push(result);
            }
        }

        // Run teardown requests
        for teardown_request_id in &test_suite.teardown_requests {
            if let Some(request) = collection.requests.iter().find(|r| r.id == *teardown_request_id)
            {
                let _response = self.execute_request(request).await?;
            }
        }

        Ok(results)
    }

    async fn evaluate_assertions(
        &self,
        assertions: &[Assertion],
        response: &ApiResponse,
    ) -> Result<Vec<AssertionResult>> {
        let mut results = Vec::new();

        for assertion in assertions {
            let result = match assertion {
                Assertion::StatusCode(expected) => {
                    let passed = response.status_code == *expected;
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("Status code is {}", expected)
                        } else {
                            format!(
                                "Expected status code {}, got {}",
                                expected, response.status_code
                            )
                        },
                    }
                }
                Assertion::StatusCodeRange { min, max } => {
                    let passed = response.status_code >= *min && response.status_code <= *max;
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!(
                                "Status code {} is in range {}-{}",
                                response.status_code, min, max
                            )
                        } else {
                            format!(
                                "Status code {} is not in range {}-{}",
                                response.status_code, min, max
                            )
                        },
                    }
                }
                Assertion::HeaderExists(header_name) => {
                    let passed = response.headers.contains_key(header_name);
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("Header '{}' exists", header_name)
                        } else {
                            format!("Header '{}' does not exist", header_name)
                        },
                    }
                }
                Assertion::HeaderEquals { header, value } => {
                    let actual_value = response.headers.get(header);
                    let passed = actual_value == Some(value);
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("Header '{}' equals '{}'", header, value)
                        } else {
                            format!(
                                "Header '{}' expected '{}', got '{}'",
                                header,
                                value,
                                actual_value.map_or("(not set)".to_string(), |v| v.clone())
                            )
                        },
                    }
                }
                Assertion::HeaderContains { header, substring } => {
                    let passed = response
                        .headers
                        .get(header)
                        .map(|v| v.contains(substring))
                        .unwrap_or(false);

                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("Header '{}' contains '{}'", header, substring)
                        } else {
                            format!("Header '{}' does not contain '{}'", header, substring)
                        },
                    }
                }
                Assertion::BodyContains(substring) => {
                    let body_text = self.response_body_as_text(&response.body);
                    let passed = body_text.contains(substring);
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("Response body contains '{}'", substring)
                        } else {
                            format!("Response body does not contain '{}'", substring)
                        },
                    }
                }
                Assertion::BodyEquals(expected) => {
                    let body_text = self.response_body_as_text(&response.body);
                    let passed = body_text.trim() == expected.trim();
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            "Response body matches expected value".to_string()
                        } else {
                            "Response body does not match expected value".to_string()
                        },
                    }
                }
                Assertion::BodyJsonPath { path, expected } => {
                    let passed = match &response.body {
                        ResponseBody::JSON(json) => {
                            self.evaluate_json_path(json, path, expected)?
                        }
                        _ => false,
                    };
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("JSON path '{}' matches expected value", path)
                        } else {
                            format!("JSON path '{}' does not match expected value", path)
                        },
                    }
                }
                Assertion::ResponseTime { max_ms } => {
                    let response_time_ms = response.response_time.as_millis() as u64;
                    let passed = response_time_ms <= *max_ms;
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!(
                                "Response time {}ms is within limit of {}ms",
                                response_time_ms, max_ms
                            )
                        } else {
                            format!(
                                "Response time {}ms exceeds limit of {}ms",
                                response_time_ms, max_ms
                            )
                        },
                    }
                }
                Assertion::ContentType(expected) => {
                    let actual = response
                        .headers
                        .get("content-type")
                        .or_else(|| response.headers.get("Content-Type"));
                    let passed = actual.map(|ct| ct.contains(expected)).unwrap_or(false);
                    AssertionResult {
                        assertion: assertion.clone(),
                        passed,
                        message: if passed {
                            format!("Content-Type contains '{}'", expected)
                        } else {
                            let actual_str = actual.map_or("(not set)".to_string(), |v| v.clone());
                            format!(
                                "Content-Type does not contain '{}', got '{}'",
                                expected, actual_str
                            )
                        },
                    }
                }
            };

            results.push(result);
        }

        Ok(results)
    }

    fn response_body_as_text(&self, body: &ResponseBody) -> String {
        match body {
            ResponseBody::Text(text) => text.clone(),
            ResponseBody::JSON(json) => serde_json::to_string_pretty(json).unwrap_or_default(),
            ResponseBody::Binary(_) => "(binary data)".to_string(),
            ResponseBody::Empty => "".to_string(),
        }
    }

    fn evaluate_json_path(
        &self,
        json: &serde_json::Value,
        path: &str,
        expected: &serde_json::Value,
    ) -> Result<bool> {
        // Simple JSON path implementation - could be expanded with a proper JSONPath library
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = json;

        for part in parts {
            if part.is_empty() {
                continue;
            }

            if let Some(array_start) = part.find('[') {
                let field_name = &part[..array_start];
                let array_end = part.find(']').ok_or_else(|| anyhow!("Invalid array syntax"))?;
                let index_str = &part[array_start + 1..array_end];
                let index: usize =
                    index_str.parse().map_err(|_| anyhow!("Invalid array index: {}", index_str))?;

                current = current
                    .get(field_name)
                    .and_then(|v| v.get(index))
                    .ok_or_else(|| anyhow!("Path not found: {}", path))?;
            } else {
                current = current.get(part).ok_or_else(|| anyhow!("Path not found: {}", path))?;
            }
        }

        Ok(current == expected)
    }

    pub async fn import_postman_collection(&self, postman_json: &str) -> Result<Uuid> {
        let postman_data: serde_json::Value = serde_json::from_str(postman_json)
            .map_err(|e| anyhow!("Failed to parse Postman collection: {}", e))?;

        let collection_name =
            postman_data["info"]["name"].as_str().unwrap_or("Imported Collection").to_string();

        let description = postman_data["info"]["description"].as_str().map(|s| s.to_string());

        let mut api_collection = ApiCollection {
            id: Uuid::new_v4(),
            name: collection_name,
            description,
            requests: Vec::new(),
            variables: HashMap::new(),
            base_url: None,
            default_headers: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Parse variables
        if let Some(variables) = postman_data["variable"].as_array() {
            for var in variables {
                if let (Some(key), Some(value)) = (var["key"].as_str(), var["value"].as_str()) {
                    api_collection.variables.insert(key.to_string(), value.to_string());
                }
            }
        }

        // Parse requests
        if let Some(items) = postman_data["item"].as_array() {
            for item in items {
                if let Some(request) = self.parse_postman_item(item)? {
                    api_collection.requests.push(request);
                }
            }
        }

        self.create_collection(api_collection).await
    }

    fn parse_postman_item(&self, item: &serde_json::Value) -> Result<Option<ApiRequest>> {
        let name = item["name"].as_str().unwrap_or("Unnamed Request").to_string();

        let request_data = &item["request"];
        if request_data.is_null() {
            return Ok(None);
        }

        let method_str = request_data["method"].as_str().unwrap_or("GET");
        let method = match method_str.to_uppercase().as_str() {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            "PUT" => HttpMethod::PUT,
            "DELETE" => HttpMethod::DELETE,
            "PATCH" => HttpMethod::PATCH,
            "HEAD" => HttpMethod::HEAD,
            "OPTIONS" => HttpMethod::OPTIONS,
            _ => HttpMethod::GET,
        };

        let url = if let Some(url_obj) = request_data["url"].as_object() {
            url_obj["raw"].as_str().unwrap_or("").to_string()
        } else {
            request_data["url"].as_str().unwrap_or("").to_string()
        };

        let mut headers = HashMap::new();
        if let Some(headers_array) = request_data["header"].as_array() {
            for header in headers_array {
                if let (Some(key), Some(value)) = (header["key"].as_str(), header["value"].as_str())
                {
                    headers.insert(key.to_string(), value.to_string());
                }
            }
        }

        let body = if let Some(body_data) = request_data["body"].as_object() {
            let mode = body_data["mode"].as_str().unwrap_or("raw");
            match mode {
                "raw" => body_data["raw"]
                    .as_str()
                    .map(|raw_body| RequestBody::Text(raw_body.to_string())),
                "formdata" => {
                    let mut form_data = HashMap::new();
                    if let Some(formdata_array) = body_data["formdata"].as_array() {
                        for field in formdata_array {
                            if let (Some(key), Some(value)) =
                                (field["key"].as_str(), field["value"].as_str())
                            {
                                form_data.insert(key.to_string(), value.to_string());
                            }
                        }
                    }
                    Some(RequestBody::FormData(form_data))
                }
                _ => None,
            }
        } else {
            None
        };

        Ok(Some(ApiRequest {
            id: Uuid::new_v4(),
            name,
            method,
            url,
            headers,
            query_params: HashMap::new(),
            body,
            auth: None,
            timeout: Duration::from_secs(30),
            follow_redirects: true,
            verify_ssl: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }))
    }

    pub async fn export_collection_as_postman(&self, collection_id: Uuid) -> Result<String> {
        let collections = self.collections.lock().await;
        let collection =
            collections.get(&collection_id).ok_or_else(|| anyhow!("Collection not found"))?;

        let mut postman_collection = serde_json::json!({
            "info": {
                "name": collection.name,
                "description": collection.description.as_deref().unwrap_or(""),
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [],
            "variable": []
        });

        // Add variables
        for (key, value) in &collection.variables {
            postman_collection["variable"].as_array_mut().unwrap().push(serde_json::json!({
                "key": key,
                "value": value
            }));
        }

        // Add requests
        for request in &collection.requests {
            let mut postman_request = serde_json::json!({
                "name": request.name,
                "request": {
                    "method": format!("{:?}", request.method),
                    "url": {
                        "raw": request.url,
                        "host": request.url.split('/').nth(2).unwrap_or("").split(':').next().unwrap_or(""),
                        "path": request.url.split('/').skip(3).collect::<Vec<_>>()
                    },
                    "header": [],
                    "body": {
                        "mode": "raw",
                        "raw": ""
                    }
                }
            });

            // Add headers
            for (key, value) in &request.headers {
                postman_request["request"]["header"].as_array_mut().unwrap().push(
                    serde_json::json!({
                        "key": key,
                        "value": value
                    }),
                );
            }

            // Add body
            if let Some(body) = &request.body {
                match body {
                    RequestBody::Text(text) => {
                        postman_request["request"]["body"]["raw"] =
                            serde_json::Value::String(text.clone());
                    }
                    RequestBody::JSON(json) => {
                        postman_request["request"]["body"]["raw"] = serde_json::Value::String(
                            serde_json::to_string_pretty(json).unwrap_or_default(),
                        );
                    }
                    RequestBody::FormData(form) => {
                        postman_request["request"]["body"]["mode"] =
                            serde_json::Value::String("formdata".to_string());
                        let mut formdata = Vec::new();
                        for (key, value) in form {
                            formdata.push(serde_json::json!({
                                "key": key,
                                "value": value,
                                "type": "text"
                            }));
                        }
                        postman_request["request"]["body"]["formdata"] =
                            serde_json::Value::Array(formdata);
                    }
                    RequestBody::Binary(_) => {
                        postman_request["request"]["body"]["mode"] =
                            serde_json::Value::String("file".to_string());
                    }
                }
            }

            postman_collection["item"].as_array_mut().unwrap().push(postman_request);
        }

        Ok(serde_json::to_string_pretty(&postman_collection)?)
    }

    pub async fn generate_curl_command(&self, request: &ApiRequest) -> Result<String> {
        let mut command = format!("curl -X {:?} '{}'", request.method, request.url);

        // Add headers
        for (key, value) in &request.headers {
            command.push_str(&format!(" -H '{}: {}'", key, value));
        }

        // Add authentication
        if let Some(auth) = &request.auth {
            match auth {
                Authentication::Bearer(token) => {
                    command.push_str(&format!(" -H 'Authorization: Bearer {}'", token));
                }
                Authentication::Basic { username, password } => {
                    command.push_str(&format!(" -u '{}:{}'", username, password));
                }
                Authentication::ApiKey { key, value } => {
                    command.push_str(&format!(" -H '{}: {}'", key, value));
                }
                Authentication::OAuth2(token) => {
                    command.push_str(&format!(" -H 'Authorization: Bearer {}'", token));
                }
            }
        }

        // Add body
        if let Some(body) = &request.body {
            match body {
                RequestBody::Text(text) => {
                    command.push_str(&format!(" -d '{}'", text.replace("'", "'\\''")));
                }
                RequestBody::JSON(json) => {
                    let json_string = serde_json::to_string(json)?;
                    command.push_str(&format!(
                        " -H 'Content-Type: application/json' -d '{}'",
                        json_string.replace("'", "'\\''")
                    ));
                }
                RequestBody::FormData(form) => {
                    for (key, value) in form {
                        command.push_str(&format!(" -d '{}={}'", key, value));
                    }
                }
                RequestBody::Binary(_) => {
                    command.push_str(" --data-binary @file.bin");
                }
            }
        }

        // Add other options
        if !request.verify_ssl {
            command.push_str(" -k");
        }

        if !request.follow_redirects {
            command.push_str(" --max-redirs 0");
        }

        command.push_str(&format!(" --max-time {}", request.timeout.as_secs()));

        Ok(command)
    }

    pub async fn get_response_history(&self) -> Result<Vec<ApiResponse>> {
        let history = self.response_history.lock().await;
        Ok(history.clone())
    }

    pub async fn clear_response_history(&self) -> Result<()> {
        let mut history = self.response_history.lock().await;
        history.clear();
        Ok(())
    }

    pub async fn get_collections(&self) -> Result<Vec<ApiCollection>> {
        let collections = self.collections.lock().await;
        Ok(collections.values().cloned().collect())
    }

    pub async fn get_collection(&self, id: Uuid) -> Result<ApiCollection> {
        let collections = self.collections.lock().await;
        collections.get(&id).cloned().ok_or_else(|| anyhow!("Collection not found"))
    }

    pub fn format_response_summary(&self, response: &ApiResponse) -> String {
        let status_emoji = if response.status_code >= 200 && response.status_code < 300 {
            "✅"
        } else if response.status_code >= 400 && response.status_code < 500 {
            "❌"
        } else if response.status_code >= 500 {
            "💥"
        } else {
            "ℹ️"
        };

        format!(
            "{} {} {} • {}ms • {} bytes",
            status_emoji,
            response.status_code,
            response.status_text,
            response.response_time.as_millis(),
            response.size_bytes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_tester_creation() {
        let _api_tester = ApiTester::new();
        // Creation should not panic
    }

    #[test]
    fn test_response_body_parsing() {
        let api_tester = ApiTester::new();

        let headers = HashMap::from([("content-type".to_string(), "application/json".to_string())]);

        let json_body = b"{\"message\": \"hello\"}";
        let result = api_tester.parse_response_body(&headers, json_body.to_vec()).unwrap();

        match result {
            ResponseBody::JSON(json) => {
                assert_eq!(json["message"], "hello");
            }
            _ => panic!("Expected JSON response body"),
        }
    }

    #[tokio::test]
    async fn test_create_collection() {
        let api_tester = ApiTester::new();

        let collection = ApiCollection {
            id: Uuid::new_v4(),
            name: "Test Collection".to_string(),
            description: Some("Test description".to_string()),
            requests: Vec::new(),
            variables: HashMap::new(),
            base_url: Some("https://api.example.com".to_string()),
            default_headers: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let collection_id = api_tester.create_collection(collection).await.unwrap();

        let retrieved = api_tester.get_collection(collection_id).await.unwrap();
        assert_eq!(retrieved.name, "Test Collection");
    }
}
