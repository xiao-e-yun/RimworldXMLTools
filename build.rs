use std::io;

fn main() -> io::Result<()> {
    #[cfg(windows)]
    {
        use winresource::WindowsResource;

        // parse the icon file and generate the icon
        let icon = generate_icon("assets/icon.png");

        // add the icon to the resources
        WindowsResource::new().set_icon(icon).compile()?;
    }
    Ok(())
}

#[cfg(windows)]
fn generate_icon(from: &str) -> &'static str {
    use {
        ico::{IconDir, IconDirEntry, IconImage, ResourceType},
        std::fs::File,
    };

    let icon = "assets/.favicon.ico";

    let mut icon_dir = IconDir::new(ResourceType::Icon);

    let file = File::open(from).unwrap();
    let image = IconImage::read_png(file).unwrap();
    icon_dir.add_entry(IconDirEntry::encode(&image).unwrap());

    let file = File::create(icon).unwrap();
    icon_dir.write(file).unwrap();

    icon
}
