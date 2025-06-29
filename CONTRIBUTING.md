# Guide for Starters
## Ideas
If you have an idea to improve the bot, just request it as a feature in an issue.

Your issue should ideally contain:
1. Your request (what you want to see added)
2. Your reasoning (why factorion should have this, not necessary if obvious)
3. Resources (links to further information of the concept, more for new mathematical constructs, like termials, arcfactorials and similar)
## Code
If you want to work on an issue, just fork the repo and start.
Don't be intimidated by the size. If you are not able to write a part of a feature (be it the parsing, the math, or anything else) or only able to write one part, that's ok.
Someone else can continue your work.

If you are confused, where in the code to work, [Code Structure](#code-structure) has an overview.

When creating a PR (requesting a review), just make sure:
1. That the code compiles (`cargo build`)
2. That your addition is tested (add unit tests and `cargo test`)
3. That Clippy doesn't complain about your code (if possible, `cargo clippy`)
4. To bump the version number (SemVer-ish, if you're unsure, just ask)
## Math
If you are not comfortable writing code but know some math, you can still contribute.
Just write a comment on the issue in question with the formula and your reasoning.
That is, in many cases, already a lot of help.

The math with reasoning and derivation is separately documented in [Math](MATH.md).
# Code Structure
## Modules
### General server stuff and API
- `main`: Program loop, executing steps, data saving/reading
- `reddit_api`: Interacting with Reddit (getting the comments, sending replies)
- `influxdb`: Sending stats
### Processing of comments
- `reddit_comment`: Executing steps for individual comments, comment metadata, reply arrangement (notes), most of the tests
- `parse`: Finding factorials in comments (with skipping URLs and spoilers), parsing numbers
- `calculation_task`: Calculating factorials in different formats (including nested), simple math
- `calculation_result`: Formatting of factorial results (different representations)
### The base math
- `math`: The mathematical formulas implemented
