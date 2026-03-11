#!/usr/bin/env bats

setup() {
    SCRIPT_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")" && pwd)"
    SKILL_FILE="${SCRIPT_DIR}/../skills/diff-context/SKILL.md"
}

@test "skill file exists" {
    [ -f "$SKILL_FILE" ]
}

@test "has valid YAML frontmatter with name" {
    head -10 "$SKILL_FILE" | grep -q "^name: diff-context$"
}

@test "has valid YAML frontmatter with description" {
    head -10 "$SKILL_FILE" | grep -q "^description:"
}

@test "description mentions changes/review/PR" {
    description=$(sed -n '/^---$/,/^---$/p' "$SKILL_FILE" | grep "^description:")
    [[ "$description" == *"change"* ]] || [[ "$description" == *"review"* ]] || [[ "$description" == *"PR"* ]]
}

@test "instructions reference ensure-cxpak" {
    grep -q "ensure-cxpak" "$SKILL_FILE"
}

@test "instructions reference cxpak diff command" {
    grep -q "cxpak diff" "$SKILL_FILE"
}

@test "instructions mention default 50k budget" {
    grep -q "50k" "$SKILL_FILE"
}

@test "instructions mention git ref option" {
    grep -q "\-\-git-ref" "$SKILL_FILE"
}

@test "instructions tell Claude to ask for budget" {
    grep -qi "ask.*budget\|budget.*ask\|ask.*token" "$SKILL_FILE"
}

@test "instructions specify markdown format" {
    grep -q "\-\-format markdown" "$SKILL_FILE"
}
