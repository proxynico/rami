use rami::app::App;

fn main() {
    match App::new() {
        Ok(Some(mut app)) => app.run(),
        Ok(None) => {}
        Err(err) => panic!("failed to start rami: {err}"),
    }
}
