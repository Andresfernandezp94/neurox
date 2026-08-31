//! Mode detection — determines Plan/Build/Chat from user text.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Chat,
    Plan,
    Build,
}

impl Mode {
    /// Get the mode-specific system prompt instruction.
    pub fn instruction(&self) -> &'static str {
        match self {
            Mode::Plan => "## MODO ACTUAL: PLAN\nSolo investigas, analizas, propones. NO ejecutes cambios.",
            Mode::Build => "## MODO ACTUAL: BUILD\nTienes poder total. Ejecuta, instala, crea sin preguntar en cada paso.",
            Mode::Chat => "",
        }
    }
}

/// Detect the mode from user text using keyword substring matching.
/// Priority: Plan > Build > Chat (default).
pub fn detect_mode(user_text: &str) -> Mode {
    let lower = user_text.to_lowercase();

    const PLAN_KEYWORDS: &[&str] = &[
        "planea",
        "plan",
        "cómo harías",
        "qué pasos",
        "explica cómo",
        "diseña",
        "piensa",
    ];

    const BUILD_KEYWORDS: &[&str] = &[
        "hazlo",
        "ejecuta",
        "instala",
        "implementa",
        "construye",
        "crea",
        "genera",
        "configura",
        "arregla",
        "fix",
        "abre",
        "cierra",
        "mata",
        "busca",
        "revisa",
        "muestra",
        "dime",
        "lee",
        "escribe",
    ];

    if PLAN_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return Mode::Plan;
    }

    if BUILD_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return Mode::Build;
    }

    Mode::Chat
}
