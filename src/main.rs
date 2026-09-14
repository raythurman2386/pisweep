mod app;

use pisweep::cli::{parse_cli, usage, CliAction};
use pisweep::icons::AppAssets;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match parse_cli(&args) {
        Ok(CliAction::Version) => {
            println!("pisweep {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Ok(CliAction::Help) => {
            println!("{}", usage());
            return;
        }
        Ok(CliAction::Run) => {}
        Err(err) => {
            eprintln!(
                "pisweep: unrecognized argument: {}\n{}",
                err.argument,
                usage()
            );
            std::process::exit(2);
        }
    }

    let app = gpui_kit::application().with_assets(AppAssets);
    app.run(|cx| {
        gpui_kit::init(cx);
        app::init(cx);
        cx.activate(true);

        cx.spawn(async move |cx| {
            app::open_window(cx).expect("failed to open window");
        })
        .detach();
    });
}
