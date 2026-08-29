use crate::config::Config;

pub fn generate_css(cfg: &Config) -> String {
    format!(
        "
        .break-surface {{
            background-color: {};
        }}
        .dial-label {{
            font-size: 72px;
            font-weight: 800;
            color: {};
            font-family: 'JetBrains Mono', 'Fira Code', 'Monospace', monospace;
        }}
        .instruction-text {{
            font-size: 24px;
            font-weight: 500;
            color: {};
            margin-top: 16px;
            margin-bottom: 12px;
        }}
        .sub-instruction-text {{
            font-size: 16px;
            font-weight: 400;
            color: {};
            opacity: 0.8;
            margin-bottom: 32px;
        }}
        .hold-unlock-canvas {{
            margin-top: 16px;
        }}
        ",
        cfg.ui.background_color,
        cfg.ui.accent_color,
        cfg.ui.accent_color,
        cfg.ui.text_color
    )
}
