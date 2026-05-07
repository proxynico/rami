use rami::app::App;

fn main() {
    match App::new() {
        Ok(Some(mut app)) => app.run(),
        Ok(None) => {}
        Err(err) => {
            eprintln!("failed to start rami: {err}");
            std::process::exit(1);
        }
    }
}
