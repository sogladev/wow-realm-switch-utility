use arboard::Clipboard;

/// May not work
pub fn to_clipboard(string: &String) -> std::io::Result<()> {
    let mut clipboard = Clipboard::new().expect("Failed to create clipboard");
    clipboard.set_text(string).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to write to clipboard",
        )
    })
}
