use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = req["method"].as_str().unwrap_or("");
        let id = req.get("id").cloned();

        // Notifications have no id — skip response
        if id.is_none() {
            continue;
        }

        let response = match method {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": { "name": "mock-server", "version": "1.0.0" },
                    "capabilities": { "tools": {} }
                }
            }),
            "tools/list" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "add",
                            "description": "Add two numbers",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "a": { "type": "number" },
                                    "b": { "type": "number" }
                                },
                                "required": ["a", "b"]
                            }
                        },
                        {
                            "name": "echo",
                            "description": "Echo input",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "input": { "type": "string" }
                                },
                                "required": ["input"]
                            }
                        },
                        {
                            "name": "slow",
                            "description": "Delayed response",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "delay_ms": { "type": "integer" }
                                },
                                "required": ["delay_ms"]
                            }
                        },
                        {
                            "name": "error",
                            "description": "Force an error",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "counter",
                            "description": "Stateful counter",
                            "inputSchema": { "type": "object", "properties": {} }
                        }
                    ]
                }
            }),
            "tools/call" => {
                let params = &req["params"];
                let tool_name = params["name"].as_str().unwrap_or("");
                let arguments = &params["arguments"];

                match tool_name {
                    "add" => {
                        let a = arguments["a"].as_f64().unwrap_or(0.0);
                        let b = arguments["b"].as_f64().unwrap_or(0.0);
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": format!("{}", a + b) }],
                                "isError": false
                            }
                        })
                    }
                    "echo" => {
                        let input = arguments["input"].as_str().unwrap_or("");
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": input }],
                                "isError": false
                            }
                        })
                    }
                    "slow" => {
                        let ms = arguments["delay_ms"].as_u64().unwrap_or(1000);
                        std::thread::sleep(std::time::Duration::from_millis(ms));
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": "done" }],
                                "isError": false
                            }
                        })
                    }
                    "error" => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": "forced error" }],
                            "isError": true
                        }
                    }),
                    "counter" => {
                        let val = COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": format!("{}", val) }],
                                "isError": false
                            }
                        })
                    }
                    _ => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": format!("Unknown tool: {}", tool_name) }
                    }),
                }
            }
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Unknown method: {}", method) }
            }),
        };

        let mut out = stdout.lock();
        serde_json::to_writer(&mut out, &response).unwrap();
        out.write_all(b"\n").unwrap();
        out.flush().unwrap();
    }
}
