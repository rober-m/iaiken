use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: &str = "5.4";
pub const KI_LI_MIMETYPE: &str = "text/x-aiken";
const KI_STATUS: &str = "ok"; // TODO: Handle error status
const KI_IMPLEMENTATION: &str = "aiken";
const KI_IMPLEMENTATION_VERSION: &str = "0.0.1";
const KI_BANNER: &str = "Aiken Kernel v0.1.0\nCardano Smart Contract Language";
const KI_DEBUGGER: bool = false;
const KI_LI_NAME: &str = "aiken";
const KI_LI_VERSION: &str = "0.0.1"; //TODO: Change to actual Aiken version
const KI_LI_FILE_EXT: &str = ".ak";

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KernelInfoRequest {}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KernelInfoReply {
    pub status: String, // 'ok' if the request succeeded or 'error', with error information
    pub protocol_version: String, // Version of messaging protocol. Format X.Y.Z
    pub implementation: String, // The kernel implementation name
    pub implementation_version: String, // The kernel implementation version. Format X.Y.Z
    pub language_info: LanguageInfo,
    pub banner: String, // A banner of information about the kernel
    pub debugger: bool, // if the kernel supports debugging in the notebook.
    pub help_links: Vec<HelpLink>,
    pub supported_features: Option<Vec<String>>, // A list of optional features such as 'debugger' and 'kernel subshells'.
}

// DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-info
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LanguageInfo {
    pub name: String,     // Name of the programming language that the kernel implements
    pub version: String,  // Language version number. Format X.Y.Z
    pub mimetype: String, // mimetype for script files in this language
    pub file_extension: String, // Extension including the dot, e.g. '.py' or '.ak'
    pub pygments_lexer: Option<String>, // Pygments lexer, for highlighting. Only needed if it differs from the 'name' field.
    pub codemirror_mode: Option<String>, // Codemirror mode, for highlighting in the notebook.. Only needed if it differs from the 'name' field.
    pub nbconvert_exporter: String,      // Nbconvert exporter
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HelpLink {
    pub text: String,
    pub url: String,
}

impl KernelInfoReply {
    pub fn new() -> Self {
        KernelInfoReply {
            status: KI_STATUS.to_string(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            implementation: KI_IMPLEMENTATION.to_string(),
            implementation_version: KI_IMPLEMENTATION_VERSION.to_string(),
            language_info: LanguageInfo {
                name: KI_LI_NAME.to_string(),
                version: KI_LI_VERSION.to_string(),
                mimetype: KI_LI_MIMETYPE.to_string(),
                file_extension: KI_LI_FILE_EXT.to_string(),
                pygments_lexer: Some(KI_LI_NAME.to_string()),
                codemirror_mode: Some(KI_LI_NAME.to_string()),
                nbconvert_exporter: "script".to_string(),
            },
            banner: KI_BANNER.to_string(),
            debugger: KI_DEBUGGER,
            help_links: vec![HelpLink {
                text: "Aiken Documentation".to_string(),
                url: "https://aiken-lang.org/".to_string(),
            }],
            supported_features: None,
        }
    }
}
