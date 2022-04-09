use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    goldboot_graphics::icon::icon_svg(true).write_to("output/icon-bg.svg")?;
    goldboot_graphics::icon::icon_svg(false).write_to("output/icon.svg")?;
    goldboot_graphics::logo::logo_svg(true).write_to("output/logo-bg.svg")?;
    goldboot_graphics::logo::logo_svg(false).write_to("output/logo.svg")?;

    Ok(())
}
