use kovi::build_bot;

fn main() {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    let bot = build_bot!(manager, command_handler, contest);
    bot.run();
}
