#!/usr/bin/env bats

setup() {
    SCRIPT_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")" && pwd)"
    PLUGIN_DIR="${SCRIPT_DIR}/.."
}

@test "plugin has README.md" {
    [ -f "${PLUGIN_DIR}/README.md" ]
}

@test "plugin has LICENSE" {
    [ -f "${PLUGIN_DIR}/LICENSE" ]
}

@test "plugin has lib/ensure-cxpak" {
    [ -x "${PLUGIN_DIR}/lib/ensure-cxpak" ]
}

@test "plugin has skills/codebase-context/SKILL.md" {
    [ -f "${PLUGIN_DIR}/skills/codebase-context/SKILL.md" ]
}

@test "plugin has skills/diff-context/SKILL.md" {
    [ -f "${PLUGIN_DIR}/skills/diff-context/SKILL.md" ]
}

@test "plugin has commands/overview.md" {
    [ -f "${PLUGIN_DIR}/commands/overview.md" ]
}

@test "plugin has commands/trace.md" {
    [ -f "${PLUGIN_DIR}/commands/trace.md" ]
}

@test "plugin has commands/diff.md" {
    [ -f "${PLUGIN_DIR}/commands/diff.md" ]
}

@test "plugin has commands/clean.md" {
    [ -f "${PLUGIN_DIR}/commands/clean.md" ]
}

@test "ensure-cxpak is executable" {
    [ -x "${PLUGIN_DIR}/lib/ensure-cxpak" ]
}

@test "no hooks directory (design says no hooks)" {
    [ ! -d "${PLUGIN_DIR}/hooks" ]
}

@test "no agents directory (design says no agents)" {
    [ ! -d "${PLUGIN_DIR}/agents" ]
}
