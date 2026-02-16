use std::{env, fs, path::PathBuf};
use xdgen::{App, Context, FluentString};

fn main() {
    let id = "com.system76.CosmicTerm";
    let ctx = Context::new("i18n", env::var("CARGO_PKG_NAME").unwrap()).unwrap();
    let app = App::new(FluentString("cosmic-terminal"))
        .comment(FluentString("comment"))
        .keywords(FluentString("keywords"));
    let output = PathBuf::from("target/xdgen");
    fs::create_dir_all(&output).unwrap();
    fs::write(
        output.join(format!("{}.desktop", id)),
        app.expand_desktop(format!("res/{}.desktop", id), &ctx)
            .unwrap(),
    )
    .unwrap();
    fs::write(
        output.join(format!("{}.metainfo.xml", id)),
        app.expand_metainfo(format!("res/{}.metainfo.xml", id), &ctx)
            .unwrap(),
    )
    .unwrap();
}
