use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("No executable process found in BPMN file")]
    NoExecutableProcess,
    #[error("Attribute error: {0}")]
    Attr(#[from] quick_xml::events::attributes::AttrError),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}
