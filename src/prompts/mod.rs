use std::{error::Error, fmt};

use rmcp::model::{GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole};
use serde_json::{Map, Value};

pub const MODULE_PURPOSE: &str = "agent prompt templates";

const BUILTIN_PROMPTS: &[PromptTemplate] = &[
    PromptTemplate {
        name: "hoi4_mod_planner",
        title: "HOI4 Mod Planner",
        description: "Turn a modding request into a game-readable HOI4 file plan.",
        mode: "Plan the requested HOI4 mod content as concrete files, identifiers, localisation keys, and validation checks.",
        arguments: &[
            PromptArgumentTemplate {
                name: "request",
                title: "Request",
                description: "The modding feature or content the agent should plan.",
                required: true,
            },
            PromptArgumentTemplate {
                name: "mod_name",
                title: "Mod Name",
                description: "Optional mod namespace or project name to use in paths and IDs.",
                required: false,
            },
        ],
    },
    PromptTemplate {
        name: "hoi4_script_writer",
        title: "HOI4 Script Writer",
        description: "Generate Paradox script for HOI4 with path and syntax constraints.",
        mode: "Write HOI4 script using stable IDs, explicit scopes, balanced braces, and matching localisation keys.",
        arguments: &[
            PromptArgumentTemplate {
                name: "request",
                title: "Request",
                description: "The script content to generate.",
                required: true,
            },
            PromptArgumentTemplate {
                name: "file_type",
                title: "File Type",
                description: "Target script type such as focus, event, decision, idea, scripted_gui, gui, or gfx.",
                required: false,
            },
        ],
    },
    PromptTemplate {
        name: "hoi4_localisation_writer",
        title: "HOI4 Localisation Writer",
        description: "Generate HOI4 localisation entries with encoding and key consistency rules.",
        mode: "Write localisation entries that match script IDs, keep language roots correct, and remind the agent to output UTF-8 BOM files when writing yml.",
        arguments: &[
            PromptArgumentTemplate {
                name: "request",
                title: "Request",
                description: "The localisation content to generate.",
                required: true,
            },
            PromptArgumentTemplate {
                name: "language",
                title: "Language",
                description: "Target language root, for example l_english or l_simp_chinese.",
                required: false,
            },
            PromptArgumentTemplate {
                name: "key_prefix",
                title: "Key Prefix",
                description: "Optional prefix for generated localisation keys.",
                required: false,
            },
        ],
    },
    PromptTemplate {
        name: "hoi4_gui_assistant",
        title: "HOI4 GUI Assistant",
        description: "Generate GUI, GFX, and scripted GUI plans for HOI4 interface work.",
        mode: "Coordinate .gui layout, .gfx sprite registration, common/scripted_guis logic, dynamic_lists, triggers, effects, and properties.",
        arguments: &[
            PromptArgumentTemplate {
                name: "request",
                title: "Request",
                description: "The interface feature to design or generate.",
                required: true,
            },
            PromptArgumentTemplate {
                name: "parent_window",
                title: "Parent Window",
                description: "Optional HOI4 parent window or view to attach to.",
                required: false,
            },
        ],
    },
    PromptTemplate {
        name: "hoi4_review",
        title: "HOI4 Mod Review",
        description: "Review generated HOI4 mod files for syntax, paths, encoding, and game readability.",
        mode: "Review generated content for invalid paths, mismatched IDs, localisation errors, missing UTF-8 BOM guidance, bad scope assumptions, and GUI/scripted_gui name mismatches.",
        arguments: &[
            PromptArgumentTemplate {
                name: "request",
                title: "Request",
                description: "The content, diff, or file list to review.",
                required: true,
            },
            PromptArgumentTemplate {
                name: "focus",
                title: "Focus",
                description: "Optional review focus such as syntax, localisation, GUI, or paths.",
                required: false,
            },
        ],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptCatalog {
    prompts: &'static [PromptTemplate],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PromptTemplate {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    mode: &'static str,
    arguments: &'static [PromptArgumentTemplate],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PromptArgumentTemplate {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptRenderError {
    UnknownPrompt(String),
    MissingRequiredArgument {
        prompt_name: &'static str,
        argument_name: &'static str,
    },
}

impl PromptCatalog {
    pub fn builtin() -> Self {
        Self {
            prompts: BUILTIN_PROMPTS,
        }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.prompts.iter().map(|prompt| prompt.name).collect()
    }

    pub fn to_mcp_prompts(&self) -> Vec<Prompt> {
        self.prompts
            .iter()
            .map(PromptTemplate::to_mcp_prompt)
            .collect()
    }

    pub fn render(
        &self,
        prompt_name: &str,
        arguments: &Map<String, Value>,
    ) -> Result<GetPromptResult, PromptRenderError> {
        let prompt = self
            .prompts
            .iter()
            .find(|candidate| candidate.name == prompt_name)
            .ok_or_else(|| PromptRenderError::UnknownPrompt(prompt_name.to_string()))?;

        let request = required_string_argument(prompt, arguments, "request")?;
        let optional_arguments = prompt
            .arguments
            .iter()
            .filter(|argument| !argument.required)
            .filter_map(|argument| {
                string_argument(arguments, argument.name)
                    .map(|value| format!("- {}: {}", argument.name, value))
            })
            .collect::<Vec<_>>();

        let optional_context = if optional_arguments.is_empty() {
            "- none".to_string()
        } else {
            optional_arguments.join("\n")
        };

        let text = format!(
            "You are RHoiScribe, a local HOI4 Modding MCP assistant.\n\
             Mode: {mode}\n\
             User request: {request}\n\
             Optional context:\n{optional_context}\n\
             Constraints:\n\
             - Produce game-readable HOI4 mod content only.\n\
             - Prefer known HOI4 folder paths, stable IDs, and matching localisation keys.\n\
             - Surface assumptions before generating files.\n\
             - Use the local RHoiScribe knowledge resources before web search.",
            mode = prompt.mode,
        );

        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
                .with_description(prompt.description),
        )
    }
}

impl PromptTemplate {
    fn to_mcp_prompt(&self) -> Prompt {
        Prompt::new(
            self.name,
            Some(self.description),
            Some(
                self.arguments
                    .iter()
                    .map(PromptArgumentTemplate::to_mcp_argument)
                    .collect(),
            ),
        )
        .with_title(self.title)
    }
}

impl PromptArgumentTemplate {
    fn to_mcp_argument(&self) -> PromptArgument {
        PromptArgument::new(self.name)
            .with_title(self.title)
            .with_description(self.description)
            .with_required(self.required)
    }
}

impl fmt::Display for PromptRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptRenderError::UnknownPrompt(prompt_name) => {
                write!(formatter, "unknown prompt `{}`", prompt_name)
            }
            PromptRenderError::MissingRequiredArgument {
                prompt_name,
                argument_name,
            } => write!(
                formatter,
                "prompt `{}` requires string argument `{}`",
                prompt_name, argument_name
            ),
        }
    }
}

impl Error for PromptRenderError {}

fn required_string_argument<'a>(
    prompt: &PromptTemplate,
    arguments: &'a Map<String, Value>,
    name: &'static str,
) -> Result<&'a str, PromptRenderError> {
    string_argument(arguments, name).ok_or(PromptRenderError::MissingRequiredArgument {
        prompt_name: prompt.name,
        argument_name: name,
    })
}

fn string_argument<'a>(arguments: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    arguments.get(name).and_then(Value::as_str)
}
