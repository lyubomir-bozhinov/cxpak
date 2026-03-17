#!/usr/bin/env bats

@test ".mcp.json exists and is valid JSON" {
    run cat plugin/.mcp.json
    [ "$status" -eq 0 ]
    echo "$output" | python3 -c "import sys, json; json.load(sys.stdin)"
}

@test ".mcp.json references ensure-cxpak-serve" {
    run cat plugin/.mcp.json
    [[ "$output" == *"ensure-cxpak-serve"* ]]
}

@test ".mcp.json uses CLAUDE_PLUGIN_ROOT variable" {
    run cat plugin/.mcp.json
    [[ "$output" == *'${CLAUDE_PLUGIN_ROOT}'* ]]
}

@test "ensure-cxpak-serve is executable" {
    [ -x plugin/lib/ensure-cxpak-serve ]
}

@test "ensure-cxpak-serve references ensure-cxpak" {
    run cat plugin/lib/ensure-cxpak-serve
    [[ "$output" == *"ensure-cxpak"* ]]
}

@test "ensure-cxpak-serve uses exec for direct stdio" {
    run cat plugin/lib/ensure-cxpak-serve
    [[ "$output" == *"exec"* ]]
}

@test "ensure-cxpak-serve passes serve --mcp" {
    run cat plugin/lib/ensure-cxpak-serve
    [[ "$output" == *"serve --mcp"* ]]
}
