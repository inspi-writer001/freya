use std::io;

use freya::App;

fn main() -> io::Result<()> {
    let mut app = App::default();
    ratatui::run(|terminal| app.run(terminal))?;

    if let Some(result) = app.last_compression_result {
        println!("{}", result);
    }

    Ok(())
}
