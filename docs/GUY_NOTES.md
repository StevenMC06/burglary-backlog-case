# Judge profile: Guy Ernest (AI-on-Cloud), from the Day 1 talk

Source: "ESEC Case Comp Day 1 Transcript - Guy.txt" (auto-transcribed, so some words are garbled; meanings below are inferred where noted).
Purpose: design and pitch to what this judge actually said he will grade. Mirror his vocabulary.

---

## 1. Who he is and why it matters
- His company (**AI-on-Cloud**; the slide header reads "NetworkAI · ESEC") is **building AI help for police departments right now**, a "similar use case." He knows the domain and will spot hand-waving.
- He hinted that the **winning team may get to work on their platform** ("if the winner team will work on a platform…"). This is a career opportunity, not just a prize.
- He grades the **technical** side. Business judges grade money; domain experts grade "is it reasonable to solve crime with this?"

## 2. His explicit grading rubric (near-verbatim)
> "What kind of **connectors and tools** did you put in place? What kind of **agents** did you define? What kind of **teams of agents** did you manage to do? It doesn't need to be only one: you can have five different teams working with 20 different data [sources]. It's **not one to one**. And what kind of **channels** you design in your architecture."

→ The deck needs one slide that makes all four layers obvious, plus an **agent × connector matrix** showing the many-to-many wiring.

## 3. His core ideas, in the order he gave them

| # | Claim | What it means for us |
|---|---|---|
| 1 | The shift is from software engineer to AI engineer to **"AI analytics"**: use AI tools to **deconstruct a business problem**. Code matters less; understanding the **business process and its modules** matters more. | Lead the pitch with the **process map and bottleneck**, not code. Code is the proof, not the story. |
| 2 | Every business process is a **flow with bottlenecks**. His own example: victim calls → someone writes it up → someone tries to solve it. The goals he named: **"solve the case, find the items, or just go back and update that we are on it."** | He named **property recovery** and **victim updates** himself. Both are in our design; say so explicitly. |
| 3 | ChatGPT alone fails in business domains: no data, no policies, no experience. **Chat is only the door.** | Never pitch "a chatbot." Pitch an architecture with a chat channel. |
| 4 | **Four layers: connectors (MCP) → agents → teams of agents → channels.** "Some people say build AI agents… I see it differently." | Use exactly these four words as slide headings. |
| 5 | MCP (Anthropic's protocol, now universal) is how the LLM reaches data and logic. Also called connectors, plugins, servers, apps. | Our connectors are real MCP servers (Rust `rmcp`). |
| 6 | LLMs are **good** at language (any language, **voice**, so police can talk hands-free), reasoning and **tool calling**. They are **bad** at **symbolic computation** and exact arithmetic ("strawberry"). | Arithmetic, scoring and constraint checks live in tools. The **cost model is a tool**: slide numbers come from code. |
| 7 | **Spectrum, not black and white:** database access is *always* a tool. "Reasoning on the likelihood this suspect is the real criminal… assembling facts into a theory" is LLM work. | The LLM forms hypotheses (series proposals); the engine checks hard facts. Stay on the right side of his line. We still don't score people, for ethics reasons. |
| 8 | **Puzzle demo** (suspects, locations, clues, a metaphor for investigation): an easy puzzle gets >95% from one LLM; a complex one gets **<50%**. **"There is a wall… it's moving but it's always there."** Don't wait for the next model (he name-checked Opus 5.5). Prompt engineering won't fix it. | Our eval should **reproduce his wall on burglary data**: single LLM vs our architecture as case count grows. |
| 9 | Puzzle architecture: one agent proposes ("can I put a blue box here?"); a **symbolic grid says yes/no**; the result drives the next iteration. **"You always have to have something behind the scenes say yes or no… grounded with data."** | This is our Series Hunter ⇄ `verify_series` ⇄ Skeptic loop. |
| 10 | **"I don't know what is the game engine in criminal investigations. But your task [is] to think about how you can verify if something is correct or not."** | **The direct challenge to teams. Our headline answer:** a constraint engine over time, space, MO and property. Quote him on the slide. |
| 11 | **Chess coach:** an LLM looking at a board photo "looks like a good answer but it's horrible"; it needs a chess engine. **MCP Apps return UI** (the board), and then the LLM explains why. "Duality." | Engine computes, LLM explains in plain language. If time allows, return the series **map/timeline as an MCP App UI** in Claude Desktop. He showed MCP Apps twice, so he values it. |
| 12 | **Forecasting:** the LLM reads the Excel, fixes date formats, calls a trained model. "**Don't ask the LLM to calculate the model in its head.**" The user adds factors ("recession") and the LLM re-calls the model. | The solvability score is a **trained model** (logistic), not a prompt. "What-if" questions in the cost model re-call the tool. |
| 13 | **Why not one agent:** bigger instructions mean more confusion and mistakes. Multiple perspectives "like a democracy": one has ideas, one **verifies**, one **approves**, one thinks strategically, one is contrarian. Each is **testable on its own**. | Proposer + **Skeptic** (contrarian) + engine (verifier) + human (approver). Each agent has its own eval. |
| 14 | **TV-team metaphor:** a leader picks specialists from a dossier: "**the muscle, the brain, the face**." Each agent is good at one thing. | Use his words "brain / muscle / face" for the roster. **Don't name the TV show** in the pitch (the transcript is ambiguous). |
| 15 | **Three design choices per agent:** (a) **instructions**: focused, non-conflicting, easy to verify, easy to test; his example is an agent that *only* receives victim calls (the "front door"), and you define what it must know and must not do; (b) **model choice**: not always the best one; Fable / Opus / Sonnet / Haiku trade speed for cost; **"with a 15% budget cut you cannot add expensive LLMs"**; (c) **MCP tools per agent**: "an agent writing evidence needs the evidence DB." | Agent cards show exactly: instructions, model and why, tools, test. Push most volume to **Haiku**. |
| 16 | **Fine-tuning** for domain language (legal, financial forensics, "follow the money"): "you really need to learn the language of the case." | **Gap in our v0.1.** Add a police shorthand glossary (POE, UNK SUSP, RP, M.O. codes) now, and a fine-tuned extractor as the roadmap item. |
| 17 | **Persona for civilian-facing agents:** calming, relaxing. | The Victim Liaison's instructions spell out tone; multilingual. |
| 18 | **Channels:** ChatGPT's breakthrough was the **interface**, not the model. Victims won't log into a police app; they **call or text**. Patrol needs **dedicated, hands-free devices**. Design how information goes in and out for each person. | Victim: SMS/text, phone, email, any language. Patrol: voice. Sergeant/detective: console. Chief: dashboard. |
| 19 | **People:** "We can't fire everybody." Savings come from **efficiency**. People have experience, move in the world and carry responsibility. **Arrests are always human decisions.** "Don't make them the bottleneck, don't wake them up in the middle of the night; let the AI work." | Savings come from overtime and attrition, not layoffs. AI does background work asynchronously; humans only at decision points. |
| 20 | **"Approval MCP"** (their platform's term): the AI sends messages to humans (approvers, field staff, victims), **non-blocking**, because people are slow and flaky. Do quick things immediately. For approvals, wait **"a couple hours, not a couple days."** | Our `approval` connector should: (1) never block the pipeline; (2) have a **timeout of about 2 h, then escalate** to the next approver; (3) let pending items continue background work. Name it "approval MCP" in his language. |

## 4. Phrases to echo in the pitch
"deconstruct the business process" · "connectors / agents / teams / channels" · "symbolic computation" · "grounded with data" · "something behind the scenes says yes or no" · "the wall" · "front door" · "the brain, the muscle, the face" · "approval MCP" · "human makes every decision, but isn't the bottleneck" · "duality: the engine computes, the LLM explains"

## 5. What will cost points with him
- A chatbot or ChatGPT wrapper, or a single mega-agent.
- An LLM doing arithmetic or "estimating" the financials.
- Using Opus/Fable everywhere ("you cannot add expensive LLMs" under a 15% budget cut).
- "Better models will fix it" (he argued the wall moves but never disappears).
- Humans blocking the pipeline, or humans removed from decisions.
- Claiming headcount cuts.
- No way to **verify** outputs.

## 6. Our v0.1 vs his talk: status
| His point | v0.1 status |
|---|---|
| Four layers | ✅ §3.1–3.4 |
| Verifier ("game engine") | ✅ `verify_series`, the headline |
| Proposer / skeptic / approver roles | ✅ |
| Model tiering | ✅ (Haiku/Sonnet/Opus per task, Batch API) |
| Tools for math | ✅ `cost-model`, trained solvability model |
| Reproduce "the wall" | ✅ planned eval, §5.4 |
| Many-to-many agent × connector | ⚠️ add matrix to architecture slide |
| Approval MCP non-blocking + ~2 h escalation | ⚠️ added to spec |
| Domain language / fine-tuning | ⚠️ glossary now, fine-tune on roadmap |
| MCP Apps UI | ⚠️ stretch: series map as a `ui://` resource |
| Victim "front door" by text/call | ⚠️ channel listed; front-door intake is outside the chosen stage, so mention it as phase 2 |
| Voice for patrol | ⚠️ phase 2 channel |
