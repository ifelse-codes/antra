# UX Test Plan — 2026-09-05

## Objective
Execute the UX test defined in UX-TEST-PROMPT.md to evaluate Antra as a complete stranger.

## Test Environment
- **Date:** 2026-09-05
- **Platform:** macOS (darwin)
- **Existing installation:** Antra 0.1.0 already installed
- **Previous test:** 2026-09-04 (reference for comparison)

## Test Phases

### Phase 1: Discovery (Website)
1. Fetch https://antra.iifelse.com using webfetch
2. Analyze landing page content
3. Answer discovery questions

### Phase 2: Installation
1. Test install script URL from website
2. Verify install script functionality
3. Document any issues

### Phase 3: First Run
1. Run `antra --help`
2. Run `antra doctor`
3. Test quick start workflow

### Phase 4: Core Usage
1. Start local server: `python3 -m http.server 8080`
2. Test `antra run --domain test.localhost -- pnpm dev`
3. Test `antra list`
4. Test `antra proxy start/stop/status`
5. Test `antra alias myapp.localhost 8080`

### Phase 5: Error Paths
1. Test port already in use
2. Test domain with no route
3. Test `antra proxy stop` when daemon not running
4. Test `antra clean`

### Phase 6: Comparison
1. Answer all comparison questions
2. Rate ease of setup (1-10)
3. Rate promise delivery (1-10)

### Phase 7: Specific Promises Check
1. Test each promise from the website
2. Document evidence for each

## Expected Output
- New test file: `tests/user-test-2026-09-05.md`
- Comparison with previous test (2026-09-04)
- Identification of regressions or improvements

## Notes
- Previous test identified issues:
  - Install URL broken (returns HTML instead of script)
  - Port auto-detection unreliable
  - No-route returns 200 instead of error
- This test will verify if these issues are resolved