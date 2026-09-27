https://antra.iifelse.com/

I want you to deeply understand this project before making any changes.

Do NOT treat this as a normal code-review task.

I want you to approach Antra as if you have just discovered an early but potentially important open-source project, and your job is to understand it from three perspectives simultaneously:

1. As a real user who has never seen the project before
2. As a senior/staff-level engineer who needs to understand how it actually works
3. As a product/design thinker who wants to help make the project significantly better

Your goal is not simply to find bugs.

Your goal is to understand what Antra is trying to become, determine whether the implementation is achieving that vision, and identify where you could meaningfully contribute.

---

## PHASE 1 — UNDERSTAND THE PROJECT

Start by exploring the repository thoroughly.

Read the important documentation, source code, configuration, examples, tests, and project structure.

Do not assume that the README tells the whole story.

Look for:

- What problem Antra is solving
- Why the project exists
- The core idea / philosophy behind it
- The intended users
- The intended developer experience
- The design language / mental model
- The architectural model
- The important abstractions
- What is deliberately different from conventional approaches
- What appears experimental vs foundational
- What is already implemented
- What is incomplete
- What is merely a prototype
- What future direction the code appears to suggest

Build a mental model of the entire system before proposing changes.

If documentation and implementation disagree, explicitly identify the discrepancy.

---

## PHASE 2 — EXPERIENCE ANTRA LIKE A NEW USER

Now use the website:

https://antra.iifelse.com/

Pretend you discovered it today and know absolutely nothing about it.

Go through the experience yourself.

Do not just inspect the source code.

Actually think through:

- What do I understand within the first 10 seconds?
- What do I think this project is?
- What is confusing?
- What makes me curious?
- Where do I lose confidence?
- What would make me want to keep exploring?
- What would make me want to contribute?
- What would make me want to use it in a real project?
- What would make me recommend it to another developer?
- What important questions remain unanswered?
- Where does the interface communicate the project's philosophy well?
- Where does the UI contradict or weaken the underlying idea?

Try to experience the project as:

- a first-time visitor
- a developer evaluating whether to adopt it
- a developer considering contributing to it
- a technically sophisticated user trying to understand its deeper model

Be critical, but do not criticize conventional patterns merely because Antra is unconventional.

Judge the project according to what it is trying to achieve.

---

## PHASE 3 — STUDY THE DESIGN LANGUAGE

Pay particular attention to the project's design language and interaction model.

Understand why the UI looks and behaves the way it does.

Identify:

- recurring visual patterns
- interaction patterns
- information hierarchy
- terminology
- metaphors
- component patterns
- motion/interaction principles
- how complexity is revealed
- how the system communicates state
- how the interface expresses the project's underlying philosophy

Do not replace the project's identity with generic SaaS/dashboard conventions.

If you suggest design improvements, they should feel like a natural evolution of Antra rather than a redesign into another product.

---

## PHASE 4 — TECHNICAL DEEP DIVE

Inspect the implementation in depth.

Understand:

- architecture
- module boundaries
- data flow
- state management
- APIs/interfaces
- persistence
- rendering
- component architecture
- error handling
- performance
- security
- developer experience
- testing strategy
- extensibility
- maintainability
- deployment/runtime assumptions

Look for both obvious and non-obvious problems.

In particular, identify places where the current implementation will become painful as the project grows.

Do not optimize prematurely.

Separate:

- actual problems
- likely future problems
- architectural risks
- technical debt
- intentional tradeoffs

---

## PHASE 5 — ADVERSARIAL TESTING

Try to break the project.

Use it in ways a normal developer might.

Look for:

- broken flows
- confusing flows
- edge cases
- inconsistent states
- unexpected inputs
- error states
- empty states
- loading states
- recovery paths
- accessibility issues
- responsive issues
- performance problems
- misleading UI
- documentation gaps
- developer onboarding friction

When you find something, reproduce it where possible.

Do not report vague observations such as "this could be better."

Explain:

WHAT happens
WHY it matters
HOW to reproduce it
WHAT you expected
WHAT actually happened
HOW you would improve it

---

## PHASE 6 — THINK LIKE A CONTRIBUTOR

Now ask yourself:

"If I were going to make one genuinely valuable contribution to Antra, what would it be?"

Do not generate a random list of improvements.

Find the highest-leverage opportunities.

Rank potential contributions by:

1. Impact on the core vision
2. Impact on users
3. Technical leverage
4. Difficulty
5. Risk of breaking existing behavior
6. Whether the contribution strengthens the project's unique identity

Look for opportunities across:

- product
- UX
- design
- architecture
- developer experience
- documentation
- testing
- performance
- tooling
- examples
- onboarding

Prioritize depth over quantity.

---

## PHASE 7 — PROPOSE, DON'T IMMEDIATELY DESTROY

Before changing substantial code, present your findings.

Give me:

### A. Your understanding of Antra

Explain in your own words:

- what Antra is
- what problem it solves
- who it is for
- what makes it different
- what you believe the project is ultimately trying to become

If your understanding is uncertain anywhere, explicitly say so.

### B. Current state assessment

Give me a concise assessment of:

- what's strong
- what's weak
- what's unfinished
- what's technically risky
- what's surprisingly good
- what's missing

### C. Top opportunities

Give me the 5–10 highest-value improvements, ranked.

For each:

- Problem
- Evidence
- Why it matters
- Proposed direction
- Expected impact
- Complexity
- Risk

### D. One recommended contribution

Choose the single contribution you believe would have the highest leverage.

Explain why you chose it over the others.

### E. Implementation plan

If you believe we should build it, provide:

- exact files/components likely to change
- architectural implications
- implementation steps
- tests required
- UX implications
- migration/backward compatibility considerations
- risks

---

## IMPORTANT RULES

Do not make changes merely to demonstrate activity.

Do not rewrite working code simply because you prefer another style.

Do not impose generic "best practices" when they conflict with Antra's philosophy.

Do not turn Antra into a conventional SaaS product.

Preserve the project's existing conceptual integrity.

Prefer small, high-leverage improvements over large rewrites.

Whenever possible, verify assumptions by inspecting the actual code and running the project rather than guessing.

If something is unclear, investigate it before concluding that it is wrong.

Most importantly:

I don't want a superficial code review.

I want you to understand Antra deeply enough that your recommendations feel like they came from someone who genuinely joined the project and wants to help build it.

Think like a founding engineer joining the project for the first time.

Understand first.

Challenge second.

Propose third.

Build only after the reasoning is solid.
