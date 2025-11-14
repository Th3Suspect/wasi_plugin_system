use plugin_api::*;

fn init() -> PluginInfo {
    PluginInfo { name: "Example Plugin".to_string(), version: "1.0.0".to_string(), author: "Suspect".to_string() }
}

fn execute(request: PluginRequest) -> PluginResponse {
    match request.command.as_str() {
        "greet" => {
            let message = format!("Hello, {}!", request.data);
            PluginResponse {
                success: true,
                data: message,
            }
        }
        "reverse" => {
            let reversed: String = request.data.chars().rev().collect();
            PluginResponse {
                success: true,
                data: reversed,
            }
        }
        "uppercase" => {
            PluginResponse {
                success: true,
                data: request.data.to_uppercase(),
            }
        }
        _ => {
            PluginResponse {
                success: false, 
                data: format!("Unknown Command: {}", request.command),
            }
        }
    }
}
export_plugin!(init, execute);