# Lekhani - Screenplay Writing Tool

.PHONY: help run dev lmstudio deepseek openai build launch clean kill fmt lint test check quick-test frontend-build frontend-serve screenshots motion

TAURI_DIR := src-tauri
FRONTEND_DIR := frontend
CARGO := cargo
CARGO_TAURI := cargo tauri
TRUNK := trunk
TRUNK_PORT := 3000
APP_BUNDLE := target/release/bundle/macos/Lekhani.app
LMSTUDIO_BASE_URL ?= http://127.0.0.1:1234/v1
LMSTUDIO_MODEL ?= nvidia/nemotron-3-nano
LMSTUDIO_TIMEOUT_SECS ?= 30
DEEPSEEK_BASE_URL ?= https://api.deepseek.com/v1
DEEPSEEK_MODEL ?= deepseek-chat
DEEPSEEK_TIMEOUT_SECS ?= 30
OPENAI_BASE_URL ?= https://api.openai.com/v1
OPENAI_MODEL ?= gpt-4.1-mini
OPENAI_TIMEOUT_SECS ?= 30

RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
BLUE := \033[0;34m
NC := \033[0m

help:
	@echo "$(GREEN)Lekhani - Screenplay Writing Tool$(NC)"
	@echo ""
	@echo "$(YELLOW)Available targets:$(NC)"
	@echo "  $(GREEN)make dev$(NC)            - Start Tauri dev mode with the Leptos frontend"
	@echo "  $(GREEN)make lmstudio$(NC)       - Start Tauri dev mode against LM Studio"
	@echo "  $(GREEN)make deepseek$(NC)       - Start Tauri dev mode against DeepSeek"
	@echo "  $(GREEN)make openai$(NC)         - Start Tauri dev mode against OpenAI"
	@echo "  $(GREEN)make run$(NC)            - Alias for dev"
	@echo "  $(GREEN)make build$(NC)          - Build the production app bundle"
	@echo "  $(GREEN)make launch$(NC)         - Rebuild and launch the macOS app bundle"
	@echo "  $(GREEN)make frontend-build$(NC) - Build the Leptos frontend only"
	@echo "  $(GREEN)make frontend-serve$(NC) - Serve the Leptos frontend only"
	@echo "  $(GREEN)make screenshots$(NC)    - Capture README screenshots from the macOS app"
	@echo "  $(GREEN)make motion$(NC)         - Capture README motion assets (mp4 + gif)"
	@echo "  $(GREEN)make fmt$(NC)            - Format the Rust workspace"
	@echo "  $(GREEN)make lint$(NC)           - Lint the Rust workspace"
	@echo "  $(GREEN)make test$(NC)           - Run workspace tests"
	@echo "  $(GREEN)make quick-test$(NC)     - Run a fast workspace compile check"
	@echo "  $(GREEN)make clean$(NC)          - Remove build artifacts"
	@echo "  $(GREEN)make kill$(NC)           - Stop running Tauri and Trunk processes"
	@echo "  $(GREEN)make check$(NC)          - Verify required tools are installed"

run: dev

dev: kill check
	@echo "$(BLUE)Starting Lekhani development server (Tauri + Leptos)...$(NC)"
	@cd $(TAURI_DIR) && $(CARGO_TAURI) dev

lmstudio: kill check
	@echo "$(BLUE)Starting Lekhani with LM Studio ($(LMSTUDIO_MODEL))...$(NC)"
	@cd $(TAURI_DIR) && env LEKHANI_LLM_PROVIDER=lmstudio LEKHANI_LMSTUDIO_BASE_URL="$(LMSTUDIO_BASE_URL)" LEKHANI_LMSTUDIO_MODEL="$(LMSTUDIO_MODEL)" LEKHANI_LMSTUDIO_TIMEOUT_SECS="$(LMSTUDIO_TIMEOUT_SECS)" $(CARGO_TAURI) dev

deepseek: kill check
	@echo "$(BLUE)Starting Lekhani with DeepSeek ($(DEEPSEEK_MODEL))...$(NC)"
	@cd $(TAURI_DIR) && env LEKHANI_LLM_PROVIDER=deepseek LEKHANI_OPENAI_BASE_URL="$(DEEPSEEK_BASE_URL)" LEKHANI_OPENAI_MODEL="$(DEEPSEEK_MODEL)" LEKHANI_OPENAI_TIMEOUT_SECS="$(DEEPSEEK_TIMEOUT_SECS)" LEKHANI_OPENAI_API_KEY="$(DEEPSEEK_API_KEY)" $(CARGO_TAURI) dev

openai: kill check
	@echo "$(BLUE)Starting Lekhani with OpenAI ($(OPENAI_MODEL))...$(NC)"
	@cd $(TAURI_DIR) && env LEKHANI_LLM_PROVIDER=openai LEKHANI_OPENAI_BASE_URL="$(OPENAI_BASE_URL)" LEKHANI_OPENAI_MODEL="$(OPENAI_MODEL)" LEKHANI_OPENAI_TIMEOUT_SECS="$(OPENAI_TIMEOUT_SECS)" LEKHANI_OPENAI_API_KEY="$(OPENAI_API_KEY)" $(CARGO_TAURI) dev

build: check frontend-build
	@echo "$(BLUE)Building Lekhani for production...$(NC)"
	@cd $(TAURI_DIR) && $(CARGO_TAURI) build

launch:
	@$(MAKE) kill
	@$(MAKE) build
	@echo "$(BLUE)Launching Lekhani app bundle...$(NC)"
	@open -n "$(APP_BUNDLE)"

clean:
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	@$(CARGO) clean
	@rm -rf dist

kill:
	@echo "$(BLUE)Killing any running Lekhani processes...$(NC)"
	@pkill -f "mathura-struggle" 2>/dev/null || true
	@pkill -f "cargo-tauri" 2>/dev/null || true
	@pkill -f "trunk" 2>/dev/null || true
	@sleep 1

fmt:
	@echo "$(BLUE)Formatting Rust workspace...$(NC)"
	@$(CARGO) fmt --all

lint:
	@echo "$(BLUE)Linting Rust workspace...$(NC)"
	@$(CARGO) clippy --workspace --all-targets

test:
	@echo "$(BLUE)Running workspace tests...$(NC)"
	@$(CARGO) test --workspace

check:
	@echo "$(BLUE)Checking dependencies...$(NC)"
	@which $(CARGO) >/dev/null 2>&1 || (echo "$(RED)Error: cargo not found$(NC)" && exit 1)
	@$(CARGO) tauri --version >/dev/null 2>&1 || (echo "$(RED)Error: cargo tauri not found. Run 'make setup'$(NC)" && exit 1)
	@which $(TRUNK) >/dev/null 2>&1 || (echo "$(RED)Error: trunk not found$(NC)" && exit 1)
	@echo "$(GREEN)All dependencies found$(NC)"

quick-test:
	@echo "$(BLUE)Running fast workspace compile check...$(NC)"
	@$(CARGO) check --workspace
	@echo "$(GREEN)Workspace compile check succeeded$(NC)"

frontend-build:
	@echo "$(BLUE)Building Leptos frontend...$(NC)"
	@cd $(FRONTEND_DIR) && env NO_COLOR=true $(TRUNK) build --release
	@echo "$(GREEN)Frontend built!$(NC)"

frontend-serve: kill check
	@echo "$(BLUE)Serving Leptos frontend on port $(TRUNK_PORT)...$(NC)"
	@cd $(FRONTEND_DIR) && env NO_COLOR=true $(TRUNK) serve --port $(TRUNK_PORT) --open false

screenshots: build
	@echo "$(BLUE)Capturing README screenshots...$(NC)"
	@./scripts/capture_readme_screenshots.sh

motion: build
	@echo "$(BLUE)Capturing README motion assets...$(NC)"
	@./scripts/capture_readme_motion.sh

.DEFAULT_GOAL := help
