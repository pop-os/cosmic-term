use cosmic::iced::clipboard::mime::AllowedMimeTypes;
use std::{borrow::Cow, error::Error, path::PathBuf, str};
use url::Url;

#[derive(Clone, Debug)]
pub struct DndDrop {
    pub paths: Vec<PathBuf>,
}

impl AllowedMimeTypes for DndDrop {
    fn allowed() -> Cow<'static, [String]> {
        Cow::from(vec![
            "x-special/gnome-copied-files".to_string(),
            "text/uri-list".to_string(),
        ])
    }
}

impl TryFrom<(Vec<u8>, String)> for DndDrop {
    type Error = Box<dyn Error>;
    fn try_from(value: (Vec<u8>, String)) -> Result<Self, Self::Error> {
        let (data, mime) = value;
        let mut paths = Vec::new();
        match mime.as_str() {
            "text/uri-list" => {
                let text = str::from_utf8(&data)?;
                for line in text.lines() {
                    let url = Url::parse(line)?;
                    match url.to_file_path() {
                        Ok(path) => paths.push(path),
                        Err(()) => Err(format!("invalid file URL {:?}", url))?,
                    }
                }
            }
            "x-special/gnome-copied-files" => {
                let text = str::from_utf8(&data)?;
                for (i, line) in text.lines().enumerate() {
                    if i != 0 {
                        let url = Url::parse(line)?;
                        match url.to_file_path() {
                            Ok(path) => paths.push(path),
                            Err(()) => Err(format!("invalid file URL {:?}", url))?,
                        }
                    }
                }
            }
            _ => Err(format!("unsupported mime type {:?}", mime))?,
        }
        Ok(Self { paths })
    }
}
