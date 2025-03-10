use kovi::build_bot;

fn main() {
    let bot = build_bot!(manager, command_handler);
    bot.run();
}
