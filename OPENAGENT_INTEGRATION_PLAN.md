# OpenAgent Integration Plan

**Created:** 2025-10-04  
**Status:** Planning Phase  
**Target:** Phase 5 Week 5-6  
**Estimated Time:** 16-20 hours

---

## 🎯 Objectives

Replace the mock agent with real OpenAgent integration to enable:
- Real LLM-powered conversations (GPT-4, Claude, local models)
- Tool calling capabilities
- Context-aware responses
- Streaming token generation
- Token usage tracking

---

## 📋 Current State Analysis

### Mock Agent Features (Working)
✅ Streaming token generation  
✅ Block detection (code, diff)  
✅ Tool request triggering  
✅ Async/await architecture  
✅ Context passing  
✅ Query cancellation

### What Needs to Change
- Replace `_mock_agent_response()` with real LLM calls
- Add LLM configuration (API keys, models)
- Implement proper streaming from LLM
- Add token counting
- Error handling for API failures
- Rate limiting
- Cost tracking

---

## 🏗️ Architecture Design

### Option 1: Direct LLM Integration (Recommended for MVP)

**Approach:** Use LLM APIs directly (OpenAI, Anthropic, etc.)

**Pros:**
- Simpler implementation
- Direct control over prompts
- Lower complexity
- Faster to implement

**Cons:**
- More manual work for tool calling
- Need to implement streaming ourselves
- Less abstraction

**Stack:**
```
Terminal UI (Rust)
    ↕ IPC
Bridge (Python)
    ↕
AgentHandler
    ↕
OpenAI/Anthropic SDK
    ↕
LLM API
```

### Option 2: OpenAgent Framework Integration

**Approach:** Use full OpenAgent framework if it exists

**Pros:**
- Framework handles tool calling
- Built-in streaming
- Agent orchestration
- Multi-agent support

**Cons:**
- Framework dependency
- More complex setup
- Need to research actual OpenAgent API

**Note:** Need to verify if OpenAgent is an existing framework or if we're building it.

---

## 🚀 Implementation Plan (Option 1 - Direct LLM)

### Phase 1: Basic LLM Integration (6 hours)

**Goal:** Get basic LLM responses working

**Tasks:**
1. Add LLM configuration system
   ```python
   # backend/openagent_terminal/llm_config.py
   class LLMConfig:
       provider: str = "openai"  # openai, anthropic, ollama
       model: str = "gpt-4-turbo-preview"
       api_key: str = None  # From env var
       base_url: Optional[str] = None  # For custom endpoints
       temperature: float = 0.7
       max_tokens: int = 2000
   ```

2. Create LLM provider abstraction
   ```python
   # backend/openagent_terminal/llm_provider.py
   class LLMProvider(ABC):
       @abstractmethod
       async def stream_completion(
           self, messages: List[dict], context: dict
       ) -> AsyncIterator[str]:
           pass
   
   class OpenAIProvider(LLMProvider):
       async def stream_completion(...):
           # Use openai.AsyncOpenAI()
   
   class AnthropicProvider(LLMProvider):
       async def stream_completion(...):
           # Use anthropic.AsyncAnthropic()
   
   class OllamaProvider(LLMProvider):
       async def stream_completion(...):
           # Use ollama API
   ```

3. Update AgentHandler to use real LLM
   ```python
   class AgentHandler:
       def __init__(self, config: LLMConfig):
           self.config = config
           self.provider = self._create_provider(config)
           self.conversation_history = []
       
       async def query(self, query_id, message, context):
           # Build messages with context
           messages = self._build_messages(message, context)
           
           # Stream from LLM
           async for token in self.provider.stream_completion(messages, context):
               yield {"content": token, "type": "text"}
   ```

**Files to Create:**
- `backend/openagent_terminal/llm_config.py`
- `backend/openagent_terminal/llm_provider.py`

**Files to Modify:**
- `backend/openagent_terminal/agent_handler.py`
- `backend/openagent_terminal/bridge.py` (pass config)

**Dependencies:**
```toml
# backend/pyproject.toml or requirements.txt
openai >= 1.3.0
anthropic >= 0.7.0  # optional
ollama >= 0.1.0  # optional
tiktoken >= 0.5.0  # token counting
```

---

### Phase 2: Conversation Context (4 hours)

**Goal:** Maintain conversation history and provide rich context

**Tasks:**
1. Build system prompt with environment context
   ```python
   def _build_system_prompt(self, context: dict) -> str:
       env = context.get("environment", {})
       
       return f"""You are OpenAgent-Terminal AI Assistant, running inside a terminal emulator.
       
       Current Environment:
       - Working Directory: {env.get('cwd')}
       - Platform: {env.get('system', {}).get('platform')}
       - Shell: {env.get('system', {}).get('shell')}
       - Git Branch: {env.get('git', {}).get('branch', 'N/A')}
       
       You have access to tools for file operations, shell commands, and more.
       Always provide clear, actionable responses.
       """
   ```

2. Manage conversation history
   ```python
   class AgentHandler:
       def __init__(self, config):
           self.conversation_history = []
           self.max_history = 10  # Keep last 10 exchanges
       
       def _build_messages(self, message, context):
           messages = [
               {"role": "system", "content": self._build_system_prompt(context)}
           ]
           
           # Add history
           for msg in self.conversation_history[-self.max_history:]:
               messages.append(msg)
           
           # Add current message
           messages.append({"role": "user", "content": message})
           
           return messages
       
       def add_to_history(self, role, content):
           self.conversation_history.append({
               "role": role,
               "content": content
           })
   ```

3. Clear history on new session
   ```python
   async def handle_session_load(self, params):
       # When loading session, restore conversation history
       session = self.session_manager.load_session(session_id)
       
       # Convert session messages to conversation history
       self.agent_handler.conversation_history = [
           {"role": msg.role.value, "content": msg.content}
           for msg in session.messages
       ]
   ```

---

### Phase 3: Tool Calling Integration (6 hours)

**Goal:** Enable LLM to call tools (function calling)

**Tasks:**
1. Define tools for LLM
   ```python
   TOOL_DEFINITIONS = [
       {
           "name": "read_file",
           "description": "Read contents of a file",
           "parameters": {
               "type": "object",
               "properties": {
                   "path": {
                       "type": "string",
                       "description": "Path to file"
                   }
               },
               "required": ["path"]
           }
       },
       {
           "name": "write_file",
           "description": "Write content to a file",
           "parameters": {
               "type": "object",
               "properties": {
                   "path": {"type": "string"},
                   "content": {"type": "string"}
               },
               "required": ["path", "content"]
           }
       },
       # ... more tools
   ]
   ```

2. Handle function calling in stream
   ```python
   async def stream_completion(self, messages, tools=None):
       response = await self.client.chat.completions.create(
           model=self.model,
           messages=messages,
           tools=tools,
           stream=True
       )
       
       async for chunk in response:
           if chunk.choices[0].delta.content:
               yield {"type": "token", "content": chunk.choices[0].delta.content}
           elif chunk.choices[0].delta.tool_calls:
               yield {
                   "type": "tool_call",
                   "tool_name": tool_call.function.name,
                   "arguments": json.loads(tool_call.function.arguments)
               }
   ```

3. Execute tools and feed results back
   ```python
   # In agent_handler.py
   async for chunk in self.provider.stream_completion(messages, tools=TOOL_DEFINITIONS):
       if chunk["type"] == "tool_call":
           # Request tool execution via existing system
           execution_id = str(uuid.uuid4())
           result = await self.tool_handler.request_tool_execution(
               execution_id,
               chunk["tool_name"],
               chunk["arguments"]
           )
           
           # Add tool result to conversation
           messages.append({
               "role": "tool",
               "tool_call_id": execution_id,
               "content": json.dumps(result)
           })
           
           # Continue streaming with tool result
           async for token in self.provider.stream_completion(messages):
               yield token
   ```

---

### Phase 4: Error Handling & Resilience (2 hours)

**Tasks:**
1. Handle API errors gracefully
   ```python
   try:
       async for token in self.provider.stream_completion(messages):
           yield token
   except openai.APIError as e:
       yield {
           "type": "error",
           "content": f"API Error: {e.message}"
       }
   except openai.RateLimitError:
       yield {
           "type": "error",
           "content": "Rate limit reached. Please wait a moment."
       }
   ```

2. Add retry logic
   ```python
   from tenacity import retry, stop_after_attempt, wait_exponential
   
   @retry(
       stop=stop_after_attempt(3),
       wait=wait_exponential(multiplier=1, min=4, max=10)
   )
   async def _api_call_with_retry(...):
       return await self.client.chat.completions.create(...)
   ```

3. Timeout handling
   ```python
   import asyncio
   
   try:
       async with asyncio.timeout(30):  # 30 second timeout
           async for token in stream:
               yield token
   except asyncio.TimeoutError:
       yield {"type": "error", "content": "Request timed out"}
   ```

---

### Phase 5: Token Usage Tracking (2 hours)

**Goal:** Track and display token usage and costs

**Tasks:**
1. Count tokens using tiktoken
   ```python
   import tiktoken
   
   class TokenCounter:
       def __init__(self, model: str):
           self.encoding = tiktoken.encoding_for_model(model)
       
       def count_tokens(self, text: str) -> int:
           return len(self.encoding.encode(text))
       
       def count_messages(self, messages: List[dict]) -> int:
           total = 0
           for msg in messages:
               total += self.count_tokens(msg["content"])
               total += 4  # Message overhead
           return total
   ```

2. Track usage per session
   ```python
   class UsageTracker:
       def __init__(self):
           self.session_usage = {}  # session_id -> usage
       
       def record_usage(self, session_id, prompt_tokens, completion_tokens):
           if session_id not in self.session_usage:
               self.session_usage[session_id] = {
                   "prompt_tokens": 0,
                   "completion_tokens": 0,
                   "total_cost": 0.0
               }
           
           usage = self.session_usage[session_id]
           usage["prompt_tokens"] += prompt_tokens
           usage["completion_tokens"] += completion_tokens
           usage["total_cost"] += self.calculate_cost(
               prompt_tokens, completion_tokens
           )
       
       def calculate_cost(self, prompt_tokens, completion_tokens):
           # GPT-4 Turbo pricing (example)
           prompt_cost = (prompt_tokens / 1000) * 0.01
           completion_cost = (completion_tokens / 1000) * 0.03
           return prompt_cost + completion_cost
   ```

3. Display usage in UI
   ```python
   # Add to session metadata
   session.metadata.prompt_tokens = usage["prompt_tokens"]
   session.metadata.completion_tokens = usage["completion_tokens"]
   session.metadata.estimated_cost = usage["total_cost"]
   ```

---

## 🔒 Security Considerations

1. **API Key Management**
   ```python
   # Never log API keys
   # Always use environment variables
   api_key = os.environ.get("OPENAI_API_KEY")
   if not api_key:
       raise ValueError("OPENAI_API_KEY environment variable not set")
   ```

2. **Rate Limiting**
   ```python
   from aiolimiter import AsyncLimiter
   
   class RateLimitedProvider:
       def __init__(self):
           # 3 requests per second
           self.limiter = AsyncLimiter(3, 1)
       
       async def stream_completion(self, ...):
           async with self.limiter:
               return await self._do_completion(...)
   ```

3. **Input Validation**
   ```python
   def validate_message(self, message: str) -> str:
       # Limit message length
       if len(message) > 10000:
           raise ValueError("Message too long (max 10,000 chars)")
       
       # Remove control characters
       return "".join(char for char in message if char.isprintable() or char.isspace())
   ```

---

## 📊 Testing Strategy

### Unit Tests
```python
# test_llm_provider.py
async def test_openai_provider():
    provider = OpenAIProvider(api_key="test-key")
    messages = [{"role": "user", "content": "Hello"}]
    
    tokens = []
    async for token in provider.stream_completion(messages):
        tokens.append(token)
    
    assert len(tokens) > 0
```

### Integration Tests
```bash
# Test with real API (using test keys)
export OPENAI_API_KEY="sk-test..."
python -m pytest tests/test_integration.py -v
```

### Mock Testing
```python
# Use VCR.py to record/replay API responses
import vcr

@vcr.use_cassette('fixtures/openai_response.yaml')
async def test_agent_query():
    agent = AgentHandler(config)
    response = await agent.query("test", "Hello", {})
    assert response is not None
```

---

## 🎛️ Configuration

### Environment Variables
```bash
# .env file
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...

# Model selection
LLM_PROVIDER=openai  # openai, anthropic, ollama
LLM_MODEL=gpt-4-turbo-preview
LLM_TEMPERATURE=0.7
LLM_MAX_TOKENS=2000

# Optional: Custom endpoints
OPENAI_BASE_URL=https://api.openai.com/v1
OLLAMA_BASE_URL=http://localhost:11434
```

### Config File
```toml
# ~/.config/openagent-terminal/config.toml
[llm]
provider = "openai"
model = "gpt-4-turbo-preview"
temperature = 0.7
max_tokens = 2000
stream = true

[llm.openai]
# API key from environment variable
api_key_env = "OPENAI_API_KEY"

[llm.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-3-sonnet-20240229"

[llm.ollama]
base_url = "http://localhost:11434"
model = "llama2"
```

---

## 📈 Success Metrics

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Response latency | < 2s first token | Time to first yield |
| Streaming smooth | < 50ms/token | Token interval timing |
| Tool calling works | 100% | Integration tests |
| Error recovery | Graceful | Error handling tests |
| Token tracking | ±5% accuracy | Compare with API response |
| Cost estimation | ±10% accuracy | Compare with billing |

---

## 🚀 Rollout Plan

### Phase 1: Development (Week 5)
- Implement basic LLM integration
- Test with OpenAI API
- Add configuration system

### Phase 2: Testing (Week 6 Day 1-3)
- Integration testing
- Error handling verification
- Performance testing

### Phase 3: Beta (Week 6 Day 4-5)
- Enable for testing
- Gather feedback
- Fix issues

### Phase 4: Production (Week 6 Day 6-7)
- Enable by default
- Update documentation
- Announce feature

---

## 💰 Cost Estimation

**Development:**
- OpenAI API testing: ~$5-10 (using GPT-4 Turbo)
- Development tokens: ~100K tokens = ~$3

**Per User (Monthly):**
- Light use (10 queries/day): ~$5-10
- Medium use (50 queries/day): ~$25-40
- Heavy use (200 queries/day): ~$100-150

**Mitigation:**
- Support local models (Ollama)
- Token limits per session
- Usage warnings
- Cost display in UI

---

## 📚 Dependencies

```txt
# Python requirements
openai>=1.3.0
anthropic>=0.7.0  # Optional
tiktoken>=0.5.0
tenacity>=8.2.0
aiolimiter>=1.1.0
python-dotenv>=1.0.0
```

---

## 🎯 Next Steps

1. ✅ Create this design document
2. ⏳ Implement LLMConfig and LLMProvider abstraction
3. ⏳ Test with OpenAI API
4. ⏳ Update AgentHandler to use real LLM
5. ⏳ Add token counting
6. ⏳ Implement tool calling
7. ⏳ Add error handling
8. ⏳ Write tests
9. ⏳ Update documentation
10. ⏳ Deploy to production

**Estimated Total Time:** 16-20 hours  
**Target Completion:** End of Phase 5 Week 6

---

**Status:** 📋 Design Complete - Ready for Implementation  
**Next Action:** Implement LLMConfig and LLMProvider classes
