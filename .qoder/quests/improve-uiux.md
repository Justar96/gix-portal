# UI/UX Improvement: Design Token Structure & Code Cleanup

## Objective

Refine the UI/UX by establishing a unified design token structure, eliminating inconsistencies, and removing dead code across the stylesheet architecture. This will improve maintainability, consistency, and visual coherence of the P2P Drive application.

## Problem Analysis

### Current State Assessment

#### Design Token Inconsistencies

1. **Mixed Variable Usage Patterns**
   - CSS custom properties (`var(--*)`) used in `invite-handler.scss`
   - SCSS variables (`$*`) used in all other components
   - No standardization between the two approaches

2. **Hardcoded Values Throughout Codebase**
   - Direct pixel values scattered across components (e.g., `4px`, `12px`, `16px`, `20px`)
   - Magic numbers not mapped to spacing scale
   - Font sizes defined as pixels instead of token references
   - Border radius values inconsistent (`6px`, `8px`, `10px`, `12px`, `16px`)

3. **Token Coverage Gaps**
   - Missing `$shadow-2xl` token (referenced in memory but not defined)
   - Incomplete semantic color variations
   - No standardized icon sizing tokens
   - Missing component-specific spacing tokens

4. **Inconsistent Component Patterns**
   - Multiple `@keyframes spin` definitions duplicated across files
   - Button variants use different sizing approaches
   - Modal overlay backdrop inconsistencies
   - Card elevation system not fully standardized

#### Code Quality Issues

1. **Duplicate Code**
   - Spinning animation defined in multiple files
   - Repeated mixin patterns that could be consolidated
   - Multiple implementations of similar button states

2. **Legacy CSS Custom Properties**
   - `invite-handler.scss` uses CSS variables instead of SCSS tokens
   - Creates maintenance burden and inconsistency risk

3. **Non-semantic Naming**
   - Some components use generic class names
   - Lack of BEM or consistent naming convention

## Design Requirements

### SCSS Modern Best Practices

The implementation will follow the latest SCSS (Sass) best practices as of 2024:

#### Module System (@use and @forward)

**Deprecation of @import**
- The `@import` rule is deprecated (phased out as of March 2023)
- All stylesheets must use the modern module system with `@use` and `@forward`
- Benefits: Namespacing, no global scope pollution, better performance, no duplicate imports

**@use Rule Standards**
- Import modules with explicit namespaces to prevent naming conflicts
- Each module is loaded and executed only once per compilation
- Access module members with namespace prefix (e.g., `variables.$accent`)
- Use `as *` sparingly and only for utility modules to maintain clarity
- Configure module variables at import time when needed

**@forward Rule Standards**
- Use `@forward` to create API layers and organize module hierarchies
- Place `@forward` statements before `@use` statements in files
- Add prefixes when forwarding to avoid naming collisions: `@forward "buttons" as btn-*`
- Control visibility with `show` and `hide` keywords for encapsulation
- Create index files (`_index.scss`) to aggregate related modules

**File Naming Convention**
- Partial files: Prefix with underscore `_variables.scss`, `_mixins.scss`
- Index files: Use `_index.scss` for module aggregation
- Component files: Descriptive names without underscores for entry points

**Namespace Management**
- Default namespace: Module filename (e.g., `@use 'variables'` → `variables.$accent`)
- Custom namespace: `@use 'variables' as vars` → `vars.$accent`
- No namespace: `@use 'variables' as *` → `$accent` (use sparingly)
- Avoid deep namespace nesting to maintain code readability

#### Modern SCSS Architecture Patterns

**7-1 Pattern Structure (Adapted)**
```
src/styles/
├── abstracts/        # Variables, functions, mixins
│   ├── _index.scss   # Forward all abstracts
│   ├── _variables.scss
│   ├── _mixins.scss
│   └── _functions.scss
├── base/             # Reset, typography, base elements
│   ├── _index.scss
│   └── _reset.scss
├── layout/           # Layout-specific styles
│   ├── _index.scss
│   └── _app.scss
├── components/       # Component styles
│   ├── _index.scss
│   ├── _buttons.scss
│   ├── _cards.scss
│   └── ...
└── main.scss        # Main entry point
```

**Module Load Order Best Practices**
1. Forward abstracts first (variables, mixins, functions)
2. Load base styles (reset, typography)
3. Load layout styles
4. Load component styles
5. Avoid circular dependencies

**Encapsulation Principles**
- Private members: Prefix with `-` or `_` (e.g., `$-internal-spacing`, `@mixin _private-helper`)
- Public API: Expose only necessary variables and mixins
- Use `@forward ... hide` to prevent internal implementation details from leaking

#### Performance Optimization

**Compilation Efficiency**
- Module system prevents duplicate imports automatically
- Each module compiled once regardless of usage count
- Reduces final CSS bundle size
- Faster build times compared to `@import`

**Selector Nesting Best Practices**
- Maximum nesting depth: 3 levels (avoid deep nesting)
- Use nesting to reflect DOM hierarchy, not to reduce typing
- Prefer flat selectors when nesting doesn't add semantic value
- Extract deeply nested selectors to separate rules

**Variable Scoping**
- Module-level variables are scoped to that module
- Use `!default` flag for configurable library variables
- Override variables using `@use "module" with ($var: value)`
- Avoid `!global` flag (anti-pattern in module system)

#### Code Quality Standards

**Consistent Formatting**
- Use 2-space or 4-space indentation consistently
- One selector per line in multi-selector rules
- One property per line
- Space after colons in property declarations
- Trailing semicolons on all declarations

**Commenting Standards**
- Document public API with section headers
- Explain non-obvious calculations or magic numbers
- Use `//` for inline comments (not compiled to CSS)
- Use `/* */` for comments that should appear in output

**Mixin Design Patterns**
- Accept parameters with sensible defaults
- Use `@content` for flexible, extensible mixins
- Avoid output-heavy mixins that generate large CSS
- Prefer utility mixins over monolithic style mixins

**Function Best Practices**
- Return single values or calculations
- Pure functions without side effects
- Clear parameter naming
- Document expected input types and return values

### Design Token Architecture

#### Token Tier System

Establish a three-tier token hierarchy:

**Tier 1: Primitive Tokens**
- Raw color values, base spacing units, font definitions
- Never used directly in components
- Example: `$color-indigo-500`, `$base-unit-4px`

**Tier 2: Semantic Tokens**
- Purpose-driven tokens referencing primitives
- Used throughout components
- Example: `$accent`, `$space-4`, `$text-primary`

**Tier 3: Component Tokens**
- Component-specific overrides and specializations
- Example: `$modal-overlay-blur`, `$button-min-height`

#### Token Categories to Standardize

**Color Tokens**
| Category | Scope | Purpose |
|----------|-------|---------|
| Background | `$bg-*` | Surface colors for different hierarchy levels |
| Text | `$text-*` | Typography color hierarchy |
| Border | `$border-*` | Divider and outline colors |
| Accent | `$accent-*` | Primary brand color variations |
| Semantic | `$success-*`, `$error-*`, `$warning-*`, `$info-*` | State-based colors |

**Spacing Tokens**
| Token Pattern | Value Range | Use Cases |
|--------------|-------------|-----------|
| `$space-{n}` | 0px - 80px (4px base) | Padding, margins, gaps |
| `$space-{n}-5` | Half-step values | Fine-tuning spacing |

**Typography Tokens**
| Category | Tokens | Purpose |
|----------|--------|---------|
| Size | `$text-2xs` through `$text-3xl` | Font size scale |
| Weight | `$weight-normal`, `$weight-medium`, `$weight-semibold` | Font weight variations |
| Leading | `$leading-*` | Line height options |
| Tracking | `$tracking-*` | Letter spacing |

**Effect Tokens**
| Category | Tokens | Coverage |
|----------|--------|----------|
| Shadows | `$shadow-xs` through `$shadow-2xl` | Elevation system |
| Border Radius | `$radius-none` through `$radius-full` | Corner rounding |
| Transitions | `$transition-fast`, `$transition-base`, `$transition-slow` | Animation timing |
| Blur | `$blur-*` | Backdrop effects |

#### Missing Tokens to Add

**Icon Sizing**
```
$icon-xs: 12px
$icon-sm: 16px
$icon-md: 20px
$icon-lg: 24px
$icon-xl: 32px
```

**Component-Specific**
```
$button-min-height: 44px
$button-min-height-sm: 40px
$input-min-height: 44px
$modal-max-width: 480px
$modal-overlay-blur: 8px
$panel-width: 280px
$sidebar-width: 260px (already exists)
```

**Missing Shadow Token**
```
$shadow-2xl: 0 24px 48px -12px rgba(0, 0, 0, 0.1), 0 12px 24px -8px rgba(0, 0, 0, 0.06)
```

### Code Cleanup Strategy

#### Hardcoded Value Elimination

**Identification Criteria**
- Any direct pixel or rem value not referencing a token
- Repeated numeric values across multiple files
- Inline color definitions (hex or rgba)

**Replacement Approach**

For each hardcoded value:
1. Determine semantic purpose (spacing, color, sizing, etc.)
2. Map to existing token or create new semantic token
3. Replace all instances with token reference
4. Document token usage in variable file

**Example Transformations**

| Current (Hardcoded) | Improved (Token-based) |
|---------------------|------------------------|
| `padding: 4px;` | `padding: $space-1;` |
| `border-radius: 16px;` | `border-radius: $radius-2xl;` |
| `font-size: 20px;` | `font-size: $text-xl;` |
| `var(--bg-primary)` | `$bg-primary` |

#### Duplicate Code Removal

**Spinning Animation Consolidation**

Currently defined in:
- `_buttons.scss`
- `_cards.scss` 
- `_states.scss`
- `_invite-handler.scss`

**Solution:** 
- Keep single definition in `_mixins.scss` as `@mixin spin`
- Use `@include spin` in components
- Remove all duplicate `@keyframes spin` definitions

**Mixin Opportunities**

Identify patterns that appear 3+ times and extract to mixins:
- Interactive states (hover/active/disabled)
- Focus ring patterns
- Loading state patterns
- Empty state layouts

#### CSS Variable Migration

**Target:** `_invite-handler.scss`

Migrate from CSS custom properties to SCSS variables:

| CSS Variable | SCSS Equivalent |
|-------------|-----------------|
| `var(--bg-primary)` | `$bg-primary` |
| `var(--border-color)` | `$border-primary` |
| `var(--text-tertiary)` | `$text-tertiary` |
| `var(--color-primary)` | `$accent` |
| `var(--color-success)` | `$success` |
| `var(--color-error)` | `$error` |

### Component Consistency Requirements

#### Button System Standardization

**Size Variants**
- Default: `min-height: $button-min-height` (44px)
- Small: `min-height: $button-min-height-sm` (40px)
- Large: `min-height: $button-min-height-lg` (48px)

**State Consistency**
- All buttons use same disabled opacity pattern
- Consistent transition timing across all variants
- Standardized focus ring implementation

#### Modal System Refinement

**Overlay Standards**
- Backdrop blur: Use `$modal-overlay-blur` token
- Background opacity: Standardize to `rgba(0, 0, 0, 0.4)`
- Z-index: Use `$z-modal` consistently

**Modal Container**
- Max width: Use `$modal-max-width` token
- Border radius: Use `$radius-xl` token
- Shadow: Use `$shadow-xl` token

#### Card System Enhancement

**Elevation Levels**

| Level | Shadow Token | Use Case |
|-------|--------------|----------|
| Subtle | `$shadow-xs` | Inline cards, minimal emphasis |
| Default | `$shadow-card` | Standard cards |
| Elevated | `$shadow-lg` | Important cards, panels |
| Floating | `$shadow-xl` | Modals, popovers |
| Maximum | `$shadow-2xl` | Critical overlays |

**Interactive States**
- Hover: Consistent transform and shadow transition
- Active: Consistent pressed state
- Disabled: Consistent opacity and filter

### Naming Convention Enforcement

#### BEM-Inspired Structure

Component structure pattern:
```
.component-name { }
.component-name__element { }
.component-name--modifier { }
```

Not strictly BEM, but maintain consistency:
- Component base class uses simple name
- Sub-elements use descriptive suffixes
- State modifiers use clear names

**Examples:**
- `.modal-overlay`, `.modal-header`, `.modal-body`, `.modal-footer`
- `.btn-primary`, `.btn-secondary`, `.btn-icon`, `.btn-ghost`
- `.card-elevated`, `.card-interactive`, `.card-loading`

## Implementation Strategy

### Phase 0: SCSS Module System Verification

**Step 0.1: Audit Current Module Usage**

Verify the codebase follows modern SCSS practices:
- Confirm all files use `@use` and `@forward` (no `@import` statements)
- Check namespace patterns are consistent
- Verify index files properly forward modules
- Ensure no circular dependencies exist

**Step 0.2: Module Structure Validation**

Validate file organization:
- Confirm all partials prefixed with underscore
- Check `_index.scss` files aggregate related modules correctly
- Verify `main.scss` entry point structure
- Ensure proper load order (abstracts → base → layout → components)

**Step 0.3: Namespace Consistency Review**

Standardize namespace usage:
- Document namespace conventions for the project
- Identify instances of `as *` usage and evaluate necessity
- Ensure abstraction modules use consistent namespace patterns
- Verify no namespace collisions exist

**Current State Assessment:**

Based on codebase analysis:
- ✅ Project uses `@use` and `@forward` (modern module system)
- ✅ Proper file structure with partials and index files
- ✅ Abstracts properly exposed via `@use '../abstracts' as *`
- ⚠️  `invite-handler.scss` uses CSS variables instead of SCSS tokens (needs migration)
- ⚠️  Some hardcoded values bypass token system
- ⚠️  Duplicate animation definitions need consolidation

### Phase 1: Design Token Consolidation

**Step 1.1: Complete Variable Definitions**

Update `_variables.scss`:
- Add missing `$shadow-2xl` token
- Add icon sizing tokens
- Add component-specific dimension tokens
- Add missing blur effect tokens

**Step 1.2: Audit Current Token Usage**

Create inventory of:
- All existing SCSS variables
- All hardcoded values in component files
- Frequency of each hardcoded value occurrence

**Step 1.3: Token Mapping Document**

Create mapping table showing:
- Hardcoded value → Appropriate token
- Justification for new tokens
- Migration priority based on frequency

### Phase 2: Systematic Value Replacement

**Step 2.1: High-Impact Replacements**

Priority order:
1. Spacing values (most common: 4px, 8px, 12px, 16px, 20px, 24px)
2. Border radius values
3. Font sizes
4. Shadow definitions
5. Color values

**SCSS Best Practice Application:**
- Use namespace prefixes when accessing tokens: `variables.$space-4`
- If using `@use '../abstracts' as *`, access directly: `$space-4`
- Maintain consistent namespace usage across all component files
- Document namespace choices in component file headers

**Step 2.2: File-by-File Migration**

Process each component file:
1. `_invite-handler.scss` (CSS variable migration + hardcoded values)
2. `_presence-panel.scss` (hardcoded spacing)
3. `_buttons.scss` (standardize sizes)
4. `_modal.scss` (standardize dimensions)
5. `_cards.scss` (standardize elevation)
6. Remaining component files

**Step 2.3: Verification**

After each file:
- Visual regression testing
- Token usage validation
- No direct pixel values remain

### Phase 3: Duplicate Code Elimination

**Step 3.1: Animation Consolidation**

- Remove duplicate `@keyframes spin` from component files
- Ensure `@mixin spin` in `_mixins.scss` is used everywhere
- Add any other reusable animations to mixin library

**Modern SCSS Pattern:**
- Define keyframes in abstracts layer (`_mixins.scss` or `_animations.scss`)
- Use `@include` to apply animations in components
- Forward animations through `_index.scss` for consistent access
- Avoid defining keyframes directly in component files
- Consider creating dedicated `_animations.scss` if library grows

**Step 3.2: Mixin Extraction**

Identify and create mixins for:
- Common interactive states
- Loading patterns
- Focus management
- Truncation and ellipsis

**Modern Mixin Design:**
- Use `@content` directive for flexible mixins that accept custom styles
- Provide sensible defaults for all parameters
- Document mixin purpose, parameters, and usage examples
- Keep mixins focused and single-purpose
- Example structure:
  ```
  @mixin interactive-card($radius: $radius-lg) {
    border-radius: $radius;
    transition: all $transition-base;
    
    @content;  // Allow custom styles
    
    &:hover {
      transform: translateY(-2px);
      box-shadow: $shadow-lg;
    }
  }
  ```

**Step 3.3: Dead Code Removal**

Remove:
- Unused CSS classes (verify with component usage)
- Commented-out code blocks
- Legacy fallback styles no longer needed
- Duplicate utility classes

### Phase 4: Component Pattern Standardization

**Step 4.1: Button System Refinement**

- Ensure all button variants use same base mixin
- Standardize min-height across variants
- Consistent state transitions
- Unified disabled state handling

**Step 4.2: Modal System Enhancement**

- Standardize overlay implementation
- Consistent header/body/footer structure
- Unified animation approach
- Same z-index handling

**Step 4.3: Card System Completion**

- Complete elevation system with all shadow levels
- Standardize interactive card behaviors
- Consistent border treatment
- Unified loading state presentation

### Phase 5: Documentation & Validation

**Step 5.1: Token Documentation**

Update `_variables.scss` with:
- Clear section headers following modern SCSS commenting standards
- Usage guidelines for each token category
- Examples of proper token usage with namespace patterns
- Migration notes for deprecated patterns
- Public API documentation using `///` doc comments for SassDoc compatibility

**Documentation Format Example:**
```
/// Primary background color for main surfaces
/// @type Color
/// @example scss - Usage
///   @use 'abstracts/variables';
///   .card { background: variables.$bg-primary; }
$bg-primary: #ffffff;
```

**Step 5.2: Component Documentation**

Document standard patterns:
- Button usage guidelines with module system examples
- Modal implementation patterns
- Card elevation decision tree
- Form input standards

**Modern SCSS Documentation Standards:**
- Use SassDoc-compatible comment format (`///`) for public APIs
- Document module dependencies and namespace requirements
- Provide before/after examples for migration guidance
- Include performance notes for complex mixins or functions
- Document private vs public members clearly

**Step 5.3: Final Validation**

- No hardcoded spacing values remain
- No hardcoded color values remain
- No duplicate animation definitions
- CSS variable usage eliminated from SCSS files (except for CSS output)
- All components use standardized patterns
- Module system compliance verified
- No circular dependencies
- Namespace usage is consistent
- All `@forward` statements precede `@use` statements
- Private members properly prefixed

**SCSS Compilation Validation:**
- Build process completes without deprecation warnings
- No `@import` deprecation warnings
- Source maps generated correctly for debugging
- Output CSS size is optimized (no duplicate rules)
- Verify CSS custom properties are intentional outputs, not SCSS mistakes

## Design Token Reference

### Complete Color System

#### Background Colors
```
$bg-primary: #ffffff (pure white surfaces)
$bg-secondary: #f5f7f9 (subtle background tint)
$bg-tertiary: #eef1f4 (more visible backgrounds)
$bg-elevated: #ffffff (raised surfaces)
$bg-hover: #e8ecf0 (hover state backgrounds)
$bg-active: #dce2e8 (active/pressed backgrounds)
$bg-card: #ffffff (card surfaces)
$bg-app: #f5f7f9 (application background)
```

#### Text Colors
```
$text-primary: #1a1a1a (main text)
$text-secondary: #4a4a4a (supporting text)
$text-tertiary: #6b7280 (subtle text)
$text-muted: #9ca3af (placeholder/disabled text)
$text-disabled: #d1d5db (disabled state)
```

#### Border Colors
```
$border-primary: #c4c9cf (strong borders)
$border-secondary: #d4d9df (standard borders)
$border-subtle: #e2e6eb (subtle dividers)
$border-hover: #9ca3af (hover state borders)
$border-focus: #0a0a0a (focus indicators)
```

#### Accent Colors
```
$accent: #6366f1 (indigo primary)
$accent-soft: rgba(99, 102, 241, 0.15) (soft backgrounds)
$accent-muted: rgba(99, 102, 241, 0.6) (muted variations)
$accent-subtle: rgba(99, 102, 241, 0.08) (very subtle tints)
$accent-dark: #4f46e5 (hover/active states)
$accent-light: #818cf8 (highlights)
```

#### Semantic Colors
```
$success: #4ade80
$success-soft: rgba(74, 222, 128, 0.15)
$success-border: rgba(74, 222, 128, 0.3)

$warning: #fbbf24
$warning-soft: rgba(251, 191, 36, 0.15)
$warning-border: rgba(251, 191, 36, 0.3)

$error: #f87171
$error-soft: rgba(248, 113, 113, 0.15)
$error-border: rgba(248, 113, 113, 0.3)

$info: #60a5fa
$info-soft: rgba(96, 165, 250, 0.15)
$info-border: rgba(96, 165, 250, 0.3)
```

### Complete Spacing Scale

```
$space-0: 0
$space-px: 1px
$space-0-5: 2px
$space-1: 4px
$space-1-5: 6px
$space-2: 8px
$space-2-5: 10px
$space-3: 12px
$space-4: 16px
$space-5: 20px
$space-6: 24px
$space-7: 28px
$space-8: 32px
$space-9: 36px
$space-10: 40px
$space-12: 48px
$space-14: 56px
$space-16: 64px
$space-20: 80px
```

### Complete Shadow System

```
$shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.04)
$shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.05), 0 1px 2px rgba(0, 0, 0, 0.03)
$shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.07), 0 2px 4px -1px rgba(0, 0, 0, 0.04)
$shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.08), 0 4px 6px -2px rgba(0, 0, 0, 0.04)
$shadow-xl: 0 20px 25px -5px rgba(0, 0, 0, 0.08), 0 10px 10px -5px rgba(0, 0, 0, 0.03)
$shadow-2xl: 0 24px 48px -12px rgba(0, 0, 0, 0.1), 0 12px 24px -8px rgba(0, 0, 0, 0.06)

$shadow-card: 0 2px 8px rgba(0, 0, 0, 0.06), 0 1px 3px rgba(0, 0, 0, 0.04)
$shadow-card-hover: 0 8px 24px rgba(0, 0, 0, 0.08), 0 4px 8px rgba(0, 0, 0, 0.04)
$shadow-card-active: 0 4px 12px rgba(0, 0, 0, 0.07), 0 2px 4px rgba(0, 0, 0, 0.03)
```

### Complete Border Radius Scale

```
$radius-none: 0
$radius-sm: 4px
$radius-md: 6px
$radius-lg: 8px
$radius-xl: 10px
$radius-2xl: 12px
$radius-full: 9999px
```

### Typography System

#### Font Families
```
$font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif
$font-mono: 'SF Mono', 'Fira Code', 'JetBrains Mono', 'Consolas', monospace
```

#### Font Sizes
```
$text-2xs: 0.6875rem (11px)
$text-xs: 0.75rem (12px)
$text-sm: 0.8125rem (13px)
$text-base: 0.875rem (14px)
$text-md: 0.9375rem (15px)
$text-lg: 1rem (16px)
$text-xl: 1.125rem (18px)
$text-2xl: 1.375rem (22px)
$text-3xl: 1.75rem (28px)
```

#### Font Weights
```
$weight-normal: 400
$weight-medium: 500
$weight-semibold: 600
```

#### Line Heights
```
$leading-none: 1
$leading-tight: 1.25
$leading-snug: 1.375
$leading-normal: 1.5
$leading-relaxed: 1.625
```

#### Letter Spacing
```
$tracking-tighter: -0.03em
$tracking-tight: -0.015em
$tracking-normal: 0
$tracking-wide: 0.015em
$tracking-wider: 0.03em
```

### Transition System

#### Durations
```
$duration-instant: 50ms
$duration-fast: 100ms
$duration-base: 150ms
$duration-slow: 250ms
$duration-slower: 350ms
```

#### Easing Functions
```
$ease-default: cubic-bezier(0.4, 0, 0.2, 1)
$ease-in: cubic-bezier(0.4, 0, 1, 1)
$ease-out: cubic-bezier(0, 0, 0.2, 1)
$ease-in-out: cubic-bezier(0.4, 0, 0.2, 1)
```

#### Transition Presets
```
$transition-fast: $duration-fast $ease-default
$transition-base: $duration-base $ease-default
$transition-slow: $duration-slow $ease-default
```

### Component Dimension Tokens (New)

```
$icon-xs: 12px
$icon-sm: 16px
$icon-md: 20px
$icon-lg: 24px
$icon-xl: 32px
$icon-2xl: 48px

$button-min-height: 44px
$button-min-height-sm: 40px
$button-min-height-lg: 48px

$input-min-height: 44px

$modal-max-width: 480px
$modal-overlay-blur: 8px

$panel-width: 280px
$sidebar-width: 260px
$sidebar-width-collapsed: 64px
```

### Effect Tokens (New)

```
$blur-none: 0
$blur-sm: 4px
$blur-md: 8px
$blur-lg: 12px
$blur-xl: 16px

$glow-focus: 0 0 0 2px rgba(99, 102, 241, 0.3)
```

## Validation Criteria

### Token Compliance

**Success Metrics:**
- Zero hardcoded pixel values for spacing (except 1px borders)
- Zero hardcoded hex or rgba colors in component files
- Zero hardcoded font-size pixel values
- All border-radius values use tokens
- All shadows use token references

**Validation Method:**
- Regex search for direct pixel values
- Regex search for color values (hex, rgb, rgba)
- Manual code review of each component file

### Code Quality

**Success Metrics:**
- Single `@keyframes spin` definition in codebase
- No duplicate mixin code
- No CSS custom properties in SCSS files
- All components follow consistent naming pattern
- No unused CSS classes remain

**Validation Method:**
- Search for duplicate animation names
- Search for `var(--` pattern in SCSS files
- Component usage verification against class definitions

### Design Consistency

**Success Metrics:**
- All buttons use standardized heights
- All modals use consistent overlay treatment
- All cards use elevation system correctly
- All interactive elements have consistent states

**Validation Method:**
- Visual inspection across all UI components
- State testing (hover, active, focus, disabled)
- Accessibility validation (focus indicators, contrast)

## Migration Risk Mitigation

### Testing Strategy

**Visual Regression Testing**
- Screenshot comparison before and after token migration
- Test all component states (default, hover, active, disabled, loading)
- Verify responsive behavior at all breakpoints

**Component Isolation Testing**
- Test each component in isolation during migration
- Verify all variants and states
- Check edge cases and error states

**Module System Testing**
- Verify compilation succeeds without errors or warnings
- Check for circular dependency issues
- Validate namespace access patterns work correctly
- Test build performance (compilation time)
- Ensure source maps point to correct source files

**Integration Testing**
- Verify components work together after changes
- Test modal overlays with other elements
- Verify z-index stacking contexts

### Rollback Plan

**Version Control Strategy**
- Create feature branch for UI/UX improvements
- Commit changes file-by-file for granular control
- Tag stable states for easy rollback points

**Incremental Deployment**
- Migrate low-risk components first
- Deploy and validate before proceeding
- Roll back individual components if issues arise

### Documentation During Migration

**Change Log**
- Document each token added or modified
- Record hardcoded value → token mappings
- Note any breaking changes or visual differences

**Review Checklist**
- Token coverage verification
- Duplicate code removal confirmation
- Visual parity validation
- Performance impact assessment

## SCSS Modern Best Practices Checklist

### Module System Compliance

- [ ] No `@import` statements (all using `@use` or `@forward`)
- [ ] All partials prefixed with underscore
- [ ] Index files use `@forward` to aggregate modules
- [ ] `@forward` statements placed before `@use` statements
- [ ] Namespace usage is consistent and documented
- [ ] No circular dependencies
- [ ] Module load order follows: abstracts → base → layout → components

### Code Organization

- [ ] Variables in `abstracts/_variables.scss`
- [ ] Mixins in `abstracts/_mixins.scss`
- [ ] Functions in `abstracts/_functions.scss` (if applicable)
- [ ] Keyframe animations in abstracts layer, not components
- [ ] Component styles isolated in separate partial files
- [ ] Each component has single responsibility

### Token System

- [ ] All spacing uses token variables
- [ ] All colors use token variables
- [ ] All shadows use token variables
- [ ] All border-radius uses token variables
- [ ] All typography sizes use token variables
- [ ] All transition timings use token variables
- [ ] No hardcoded values (except 1px borders, 0 values)

### Performance

- [ ] Selector nesting depth ≤ 3 levels
- [ ] No overly complex mixins generating excessive CSS
- [ ] Module system prevents duplicate imports
- [ ] Output CSS is optimized and minimal
- [ ] Source maps work correctly for debugging

### Code Quality

- [ ] Consistent indentation (2 or 4 spaces)
- [ ] Trailing semicolons on all declarations
- [ ] One property per line
- [ ] Public API documented with `///` SassDoc comments
- [ ] Private members prefixed with `-` or `_`
- [ ] Mixins use `@content` where appropriate
- [ ] Functions are pure (no side effects)

### Maintainability

- [ ] Clear section headers with comments
- [ ] Token usage examples provided
- [ ] Migration paths documented
- [ ] Deprecated patterns identified
- [ ] No duplicate code across files
- [ ] Consistent naming conventions

## Expected Outcomes

### Maintainability Improvements

**Centralized Design Control**
- Single source of truth for all design values
- Easy theme modifications through variable updates
- Simplified onboarding for new developers
- Module system provides clear dependency tracking

**Reduced Technical Debt**
- No duplicate code to maintain
- Consistent patterns reduce cognitive load
- Clear architectural boundaries
- Modern SCSS features prevent common pitfalls

### Visual Consistency Enhancements

**Unified Design Language**
- Predictable spacing system
- Coherent elevation hierarchy
- Consistent interactive behaviors

**Professional Polish**
- No visual inconsistencies between components
- Smooth, uniform transitions
- Refined micro-interactions

### Development Velocity

**Faster Component Development**
- Reusable token library accelerates work
- Standard patterns reduce decision fatigue
- Less debugging of inconsistencies
- Module system prevents import errors

**Easier Collaboration**
- Clear conventions for all developers
- Reduced merge conflicts in styles
- Self-documenting token system
- Namespace clarity prevents naming conflicts

## Success Criteria

### Quantitative Metrics

- Hardcoded values: Reduce from ~50+ instances to 0
- Duplicate animations: Reduce from 4+ to 1
- CSS variable usage: Migrate 100% to SCSS variables (for internal use)
- Token coverage: 100% of spacing, colors, shadows use tokens
- Module system compliance: 100% `@use`/`@forward`, 0% `@import`
- Build warnings: 0 deprecation warnings

### Qualitative Assessment

- Design consistency: All components follow unified visual language
- Code readability: Improved clarity through semantic token names
- Developer confidence: Clear guidance on which tokens to use
- Visual polish: Refined, professional appearance throughout UI
- SCSS maintainability: Modern module system ensures long-term code health
- Build performance: Optimized compilation with module caching

### User Experience Impact

- Perceived quality: More polished, cohesive interface
- Visual hierarchy: Clearer information structure
- Interaction feedback: Consistent, smooth micro-interactions
- Professional appearance: Enterprise-grade UI presentation

## Alignment with User Preferences

This design adheres to established user communication and UI preferences:

**Minimal UI Styling**
- Token-based system enables clean, uncluttered design
- Avoids heavy or distracting animations
- Focus on functional, usable interface

**Micro-interactions Retention**
- Smooth transitions preserved through transition tokens
- Hover states maintained with consistent timing
- Small feedback effects standardized across components
- Professional micro-interactions enhance usability without distraction

**Professional Polish**
- Clean, professionally worded design token naming
- Simple, clear component patterns
- Functional focus over decorative elements
- Enterprise-grade visual consistency

**Implementation Notes**
- Animation consolidation removes heavy effects while preserving essential feedback
- Transition system provides smooth interactions without distraction
- Design tokens enable quick adjustments to animation timing if needed
- Consistent spacing and shadows create professional, minimal aesthetic
#### Letter Spacing
```
$tracking-tighter: -0.03em
$tracking-tight: -0.015em
$tracking-normal: 0
$tracking-wide: 0.015em
$tracking-wider: 0.03em
```

### Transition System

#### Durations
```
$duration-instant: 50ms
$duration-fast: 100ms
$duration-base: 150ms
$duration-slow: 250ms
$duration-slower: 350ms
```

#### Easing Functions
```
$ease-default: cubic-bezier(0.4, 0, 0.2, 1)
$ease-in: cubic-bezier(0.4, 0, 1, 1)
$ease-out: cubic-bezier(0, 0, 0.2, 1)
$ease-in-out: cubic-bezier(0.4, 0, 0.2, 1)
```

#### Transition Presets
```
$transition-fast: $duration-fast $ease-default
$transition-base: $duration-base $ease-default
$transition-slow: $duration-slow $ease-default
```

### Component Dimension Tokens (New)

```
$icon-xs: 12px
$icon-sm: 16px
$icon-md: 20px
$icon-lg: 24px
$icon-xl: 32px
$icon-2xl: 48px

$button-min-height: 44px
$button-min-height-sm: 40px
$button-min-height-lg: 48px

$input-min-height: 44px

$modal-max-width: 480px
$modal-overlay-blur: 8px

$panel-width: 280px
$sidebar-width: 260px
$sidebar-width-collapsed: 64px
```

### Effect Tokens (New)

```
$blur-none: 0
$blur-sm: 4px
$blur-md: 8px
$blur-lg: 12px
$blur-xl: 16px

$glow-focus: 0 0 0 2px rgba(99, 102, 241, 0.3)
```

## Validation Criteria

### Token Compliance

**Success Metrics:**
- Zero hardcoded pixel values for spacing (except 1px borders)
- Zero hardcoded hex or rgba colors in component files
- Zero hardcoded font-size pixel values
- All border-radius values use tokens
- All shadows use token references

**Validation Method:**
- Regex search for direct pixel values
- Regex search for color values (hex, rgb, rgba)
- Manual code review of each component file

### Code Quality

**Success Metrics:**
- Single `@keyframes spin` definition in codebase
- No duplicate mixin code
- No CSS custom properties in SCSS files
- All components follow consistent naming pattern
- No unused CSS classes remain

**Validation Method:**
- Search for duplicate animation names
- Search for `var(--` pattern in SCSS files
- Component usage verification against class definitions

### Design Consistency

**Success Metrics:**
- All buttons use standardized heights
- All modals use consistent overlay treatment
- All cards use elevation system correctly
- All interactive elements have consistent states

**Validation Method:**
- Visual inspection across all UI components
- State testing (hover, active, focus, disabled)
- Accessibility validation (focus indicators, contrast)

## Migration Risk Mitigation

### Testing Strategy

**Visual Regression Testing**
- Screenshot comparison before and after token migration
- Test all component states (default, hover, active, disabled, loading)
- Verify responsive behavior at all breakpoints

**Component Isolation Testing**
- Test each component in isolation during migration
- Verify all variants and states
- Check edge cases and error states

**Integration Testing**
- Verify components work together after changes
- Test modal overlays with other elements
- Verify z-index stacking contexts

### Rollback Plan

**Version Control Strategy**
- Create feature branch for UI/UX improvements
- Commit changes file-by-file for granular control
- Tag stable states for easy rollback points

**Incremental Deployment**
- Migrate low-risk components first
- Deploy and validate before proceeding
- Roll back individual components if issues arise

### Documentation During Migration

**Change Log**
- Document each token added or modified
- Record hardcoded value → token mappings
- Note any breaking changes or visual differences

**Review Checklist**
- Token coverage verification
- Duplicate code removal confirmation
- Visual parity validation
- Performance impact assessment

## SCSS Modern Best Practices Checklist

### Module System Compliance

- [ ] No `@import` statements (all using `@use` or `@forward`)
- [ ] All partials prefixed with underscore
- [ ] Index files use `@forward` to aggregate modules
- [ ] `@forward` statements placed before `@use` statements
- [ ] Namespace usage is consistent and documented
- [ ] No circular dependencies
- [ ] Module load order follows: abstracts → base → layout → components

### Code Organization

- [ ] Variables in `abstracts/_variables.scss`
- [ ] Mixins in `abstracts/_mixins.scss`
- [ ] Functions in `abstracts/_functions.scss` (if applicable)
- [ ] Keyframe animations in abstracts layer, not components
- [ ] Component styles isolated in separate partial files
- [ ] Each component has single responsibility

### Token System

- [ ] All spacing uses token variables
- [ ] All colors use token variables
- [ ] All shadows use token variables
- [ ] All border-radius uses token variables
- [ ] All typography sizes use token variables
- [ ] All transition timings use token variables
- [ ] No hardcoded values (except 1px borders, 0 values)

### Performance

- [ ] Selector nesting depth ≤ 3 levels
- [ ] No overly complex mixins generating excessive CSS
- [ ] Module system prevents duplicate imports
- [ ] Output CSS is optimized and minimal
- [ ] Source maps work correctly for debugging

### Code Quality

- [ ] Consistent indentation (2 or 4 spaces)
- [ ] Trailing semicolons on all declarations
- [ ] One property per line
- [ ] Public API documented with `///` SassDoc comments
- [ ] Private members prefixed with `-` or `_`
- [ ] Mixins use `@content` where appropriate
- [ ] Functions are pure (no side effects)

### Maintainability

- [ ] Clear section headers with comments
- [ ] Token usage examples provided
- [ ] Migration paths documented
- [ ] Deprecated patterns identified
- [ ] No duplicate code across files
- [ ] Consistent naming conventions

## Expected Outcomes

### Maintainability Improvements

**Centralized Design Control**
- Single source of truth for all design values
- Easy theme modifications through variable updates
- Simplified onboarding for new developers

**Reduced Technical Debt**
- No duplicate code to maintain
- Consistent patterns reduce cognitive load
- Clear architectural boundaries

### Visual Consistency Enhancements

**Unified Design Language**
- Predictable spacing system
- Coherent elevation hierarchy
- Consistent interactive behaviors

**Professional Polish**
- No visual inconsistencies between components
- Smooth, uniform transitions
- Refined micro-interactions

### Development Velocity

**Faster Component Development**
- Reusable token library accelerates work
- Standard patterns reduce decision fatigue
- Less debugging of inconsistencies

**Easier Collaboration**
- Clear conventions for all developers
- Reduced merge conflicts in styles
- Self-documenting token system

## Success Criteria

### Quantitative Metrics

- Hardcoded values: Reduce from ~50+ instances to 0
- Duplicate animations: Reduce from 4+ to 1
- CSS variable usage: Migrate 100% to SCSS variables
- Token coverage: 100% of spacing, colors, shadows use tokens

### Qualitative Assessment

- Design consistency: All components follow unified visual language
- Code readability: Improved clarity through semantic token names
- Developer confidence: Clear guidance on which tokens to use
- Visual polish: Refined, professional appearance throughout UI

### User Experience Impact

- Perceived quality: More polished, cohesive interface
- Visual hierarchy: Clearer information structure
- Interaction feedback: Consistent, smooth micro-interactions
- Professional appearance: Enterprise-grade UI presentation
