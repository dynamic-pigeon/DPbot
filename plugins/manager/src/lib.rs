use kovi::PluginBuilder as plugin;
use kovi::bot::runtimebot::kovi_api::SetAccessControlList;
use kovi::serde_json;
use kovi::serde_json::Value;

const PLUGINS: &[&str] = &["command_handler"];

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.json");
    let config: Value =
        serde_json::from_reader(std::fs::File::open(&config_path).unwrap()).unwrap();

    // Initialize the whitelist
    let whitelist = config["whitelist"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect::<Vec<_>>();

    for plugin_name in PLUGINS {
        bot.set_plugin_access_control(plugin_name, true).unwrap();
        bot.set_plugin_access_control_list(
            plugin_name,
            true,
            SetAccessControlList::Changes(whitelist.clone()),
        )
        .unwrap();
    }
}
