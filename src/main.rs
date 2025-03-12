use kovi::build_bot;

fn main() {
    env_logger::init();
    let bot = build_bot!(manager, command_handler, contest);
    bot.run();
}
