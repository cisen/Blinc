#!/bin/bash
# Script to publish all Blinc crates to crates.io in dependency order
# Usage: ./scripts/publish-crates.sh
#
# Note: crates.io has a burst limit of ~30 publishes before rate limiting kicks in.
# We only need a short delay between publishes to let the index update so that
# dependent crates can resolve their dependencies.

set -e

# Source the cargo registry token
if [ -f ".env.cargo" ]; then
    source .env.cargo
fi

WAIT_TIME=20  # seconds between publishes (index propagation)

# Publish order (respects dependency graph)
# Note: blinc_core has blinc_animation as dev-dep only, so core goes first
PHASE1=(blinc_macros blinc_platform blinc_icons blinc_core)
PHASE2=(blinc_animation blinc_paint blinc_svg blinc_text)
PHASE3=(blinc_theme blinc_image blinc_layout)
PHASE4=(blinc_gpu blinc_cn)
PHASE5=(blinc_platform_desktop blinc_platform_android blinc_platform_ios)
PHASE6=(blinc_app)
PHASE7=(blinc_cli)

publish_crate() {
    local crate=$1
    local version
    version=$(cargo metadata --no-deps --format-version 1 \
        | grep -o "\"name\":\"$crate\",\"version\":\"[^\"]*\"" \
        | head -1 \
        | sed 's/.*"version":"\([^"]*\)"/\1/')

    echo ""
    echo "=========================================="
    echo "Publishing $crate@$version..."
    echo "=========================================="

    # Check if this version already exists on crates.io
    if cargo search "$crate" 2>/dev/null | grep -q "^$crate = \"$version\""; then
        echo "Skipping $crate@$version (already published)"
        return 0
    fi

    if cargo publish -p "$crate" 2>&1; then
        echo "Successfully published $crate@$version"
        return 0
    else
        echo "Failed to publish $crate"
        return 1
    fi
}

wait_for_rate_limit() {
    echo ""
    echo "Waiting $WAIT_TIME seconds for index propagation..."
    sleep $WAIT_TIME
}

publish_phase() {
    local phase_name=$1
    shift
    local crates=("$@")

    echo ""
    echo "############################################"
    echo "# $phase_name"
    echo "############################################"

    for crate in "${crates[@]}"; do
        if publish_crate "$crate"; then
            wait_for_rate_limit
        fi
    done
}

echo "Starting Blinc crate publishing..."
echo "This will take approximately $((($WAIT_TIME * 17) / 60)) minutes (17 crates x ${WAIT_TIME}s index wait)."
echo ""

# Check if CARGO_REGISTRY_TOKEN is set
if [ -z "$CARGO_REGISTRY_TOKEN" ]; then
    echo "Error: CARGO_REGISTRY_TOKEN not set"
    echo "Please set it in .env.cargo or export it"
    exit 1
fi

# Start publishing
publish_phase "Phase 1: Foundation crates" "${PHASE1[@]}"
publish_phase "Phase 2: Core systems" "${PHASE2[@]}"
publish_phase "Phase 3: Higher-level systems" "${PHASE3[@]}"
publish_phase "Phase 4: GPU and components" "${PHASE4[@]}"
publish_phase "Phase 5: Platform extensions" "${PHASE5[@]}"
publish_phase "Phase 6: Application framework" "${PHASE6[@]}"
publish_phase "Phase 7: CLI" "${PHASE7[@]}"

echo ""
echo "=============================================="
echo "All crates published successfully!"
echo "=============================================="
