use std::{error::Error, fmt};

use rmcp::model::{GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole};
use serde_json::{Map, Value};

pub const MODULE_PURPOSE: &str = "agent prompt templates";

const BUILTIN_PROMPTS: &[PromptTemplate] = &[
    PromptTemplate {
        name: "hoi4_mod_planner",
        title: "HOI4 Mod Planner",
        description: "Turn a modding request into a game-readable HOI4 file plan.",
        mode: "Plan the requested HOI4 mod content as concrete files, identifiers, localisation keys, and validation checks. First mirror the user's current workspace path and naming conventions when visible.",
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
        mode: "Write HOI4 script using stable IDs, explicit scopes, balanced braces, and matching localisation keys. Match existing workspace ID, variable, focus, idea, event, and file naming style before falling back to official conventions.",
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
        mode: "Write localisation entries that match script IDs, keep language roots correct, and preserve the workspace's existing localisation folder depth and filename style before falling back to official HOI4 conventions.",
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
        mode: "Coordinate .gui layout, .gfx sprite registration, common/scripted_guis logic, dynamic_lists, triggers, effects, and properties. Learn existing GUI element, sprite, variable, and asset path conventions from the user's workspace first.",
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
        mode: "Review generated content against the user's explicit request and workspace conventions first, then official HOI4 path, syntax, encoding, scope, localisation, and GUI/scripted_gui rules.",
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
            .map(PromptTemplate::as_mcp_prompt)
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
        let optional_context = optional_context(prompt, arguments);

        let text = render_prompt_text(prompt.mode, request, &optional_context);

        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, text)])
                .with_description(prompt.description),
        )
    }
}

fn optional_context(prompt: &PromptTemplate, arguments: &Map<String, Value>) -> String {
    let optional_arguments = prompt
        .arguments
        .iter()
        .filter(|argument| !argument.required)
        .filter_map(|argument| {
            string_argument(arguments, argument.name)
                .map(|value| format!("- {}: {}", argument.name, value))
        })
        .collect::<Vec<_>>();

    if optional_arguments.is_empty() {
        "- none".to_string()
    } else {
        optional_arguments.join("\n")
    }
}

fn render_prompt_text(mode: &str, request: &str, optional_context: &str) -> String {
    format!(
        "You are RHoiScribe, a local HOI4 Modding MCP assistant.\n\
             Mode: {mode}\n\
             User request: {request}\n\
             Optional context:\n{optional_context}\n\
             Constraints:\n{constraints}",
        mode = mode,
        constraints = PROMPT_CONSTRAINTS,
    )
}

const PROMPT_CONSTRAINTS: &str = "\
             - Start by translating the user's goal into a verifiable checklist covering requested outcomes, affected files, unique IDs, expected game-readable output, and validation evidence.\n\
             - Use RED/GREEN/VERIFY for tool or content changes: RED means define or run the check that would fail before the change when feasible; GREEN means create the smallest complete game-readable change; VERIFY means rerun fresh checks and inspect generated output before saying the task is complete.\n\
             - Do not claim completion, safety, or compatibility without fresh verification evidence from the current workspace or from the generated dry-run output.\n\
             - Produce game-readable HOI4 mod content only.\n\
             - Priority order: current user request, then conventions discovered in the user's workspace, then bundled RHoiScribe resources, then official HOI4 defaults.\n\
             - Before choosing paths or names, inspect available workspace files and mirror existing folder depth, filename suffixes, tag prefixes, variable names, focus IDs, event namespaces, idea IDs, GUI element names, and localisation key style.\n\
             - For broad edits, first call index_hoi4_project to build definitions and references, then call validate_hoi4_project before writing or claiming the project is clean.\n\
             - Before creating new unique identifiers such as TAGs, focus IDs, shared or joint focus IDs, idea tokens, dynamic modifiers, country/global/state/character/MIO/project flags, variables, event namespaces, decisions, characters, scripted effects, or scripted triggers, call scan_unique_identifiers with intent=create. Use intent=reference when the user asks to reuse existing content.\n\
             - Prefer edit_hoi4_script_file for targeted changes to existing HOI4 txt/gui/gfx/lua files instead of regenerating whole files.\n\
             - Use repair_hoi4_project with dry_run=true before applying encoding, formatting, or audio fixes. If ffmpeg is required and missing, ask for user approval; only then allow dry_run=false with install_ffmpeg=true for a silent installation attempt.\n\
             - Treat generate_gui_gfx_asset as experimental. Use existing project art first unless the user approves new procedural assets; only pass approved=true after that approval, and do not use external image generation models.\n\
             - Do not force flat localisation paths. Nested paths such as localisation/simp_chinese/common/autonomy/custom_autonomy_l_simp_chinese.yml are valid when they match the workspace convention or user request; the language suffix is the normal filename convention, not a TAG naming rule.\n\
             - Keep workspace file names, folder names, script token fields, idea IDs, focus IDs, event IDs, variable names, flag names, OOB division names, and similar identifiers ASCII-only. Localisation prose and visible player text may use the target language.\n\
             - Keep speaking in the user's initial conversation language. When adding code comments, write clear English comments with no filler.\n\
             - Deliver complete usable content, not skeleton files, TODO placeholders, draft-only text, or follow-up stubs. Do not leave temporary scripts or unrelated generated files behind.\n\
             - Do not damage unrelated workspace content, reset git state, or rewrite files outside the requested scope. If the user permits commits, use Conventional Commits such as feat: add automated release binaries.\n\
             - Localisation must be finished prose, not placeholders or design notes. For focus descriptions, mission descriptions, event text, and similar player-facing prose, match any existing project narrative style and write polished long-form text unless the user explicitly asks for terse text.\n\
             - Hide implementation-only triggers/effects and helper modifiers from the player where the file type supports hidden_trigger, hidden_effect, hidden = yes, or equivalent UI omission. Do not expose calculation-only dynamic modifiers as visible player-facing modifiers.\n\
             - For focus-tree layout, do not assume x spacing 2 and y spacing 1 until checking whether interface/nationalfocusview.gui exists in the workspace or dependency mods. If custom focus spacing exists, inspect focus_spacing, positionType, and icon dimensions; ask when ambiguous and avoid overlap.\n\
             - Use discover_hoi4_environment to locate game_path, document_path, and version when local game context is needed. Before game debug launch, use validate_hoi4_debug_run and require clean document map/localisation/history folders plus a playset containing only the workspace mod and its dependencies.\n\
             - When investigating crashes or load failures, classify error.log first, correlate entries with changed paths, and use git only for analysis unless the user explicitly permits changes.\n\
             - If no workspace convention is visible, say so and fall back to HOI4-readable defaults.\n\
             - Surface assumptions before generating files.\n\
             - Use the local RHoiScribe knowledge resources before web search.";

impl PromptTemplate {
    fn as_mcp_prompt(&self) -> Prompt {
        Prompt::new(
            self.name,
            Some(self.description),
            Some(
                self.arguments
                    .iter()
                    .map(PromptArgumentTemplate::as_mcp_argument)
                    .collect(),
            ),
        )
        .with_title(self.title)
    }
}

impl PromptArgumentTemplate {
    fn as_mcp_argument(&self) -> PromptArgument {
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

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::PromptCatalog;

    #[test]
    fn rendered_prompts_require_red_green_verify_delivery() {
        let arguments = Map::from_iter([("request".to_string(), json!("add a focus branch"))]);
        let rendered = PromptCatalog::builtin()
            .render("hoi4_mod_planner", &arguments)
            .expect("prompt should render");
        let text = serde_json::to_string(&rendered).expect("prompt should serialize");

        assert!(text.contains("RED/GREEN/VERIFY"));
        assert!(text.contains("verifiable checklist"));
        assert!(text.contains("fresh verification"));
    }

    #[test]
    fn rendered_prompts_keep_workspace_conventions_above_defaults() {
        let arguments = Map::from_iter([(
            "request".to_string(),
            Value::String("write localisation".to_string()),
        )]);
        let rendered = PromptCatalog::builtin()
            .render("hoi4_localisation_writer", &arguments)
            .expect("prompt should render");
        let text = serde_json::to_string(&rendered).expect("prompt should serialize");

        assert!(text.contains("current user request"));
        assert!(text.contains("workspace"));
        assert!(text.contains("official HOI4 defaults"));
    }
}
