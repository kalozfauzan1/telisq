---
name: clarify-requirements
description: Ask clarifying questions when requirements are ambiguous before providing answers
---

# Clarification Protocol

Before answering, check if:

- The question has multiple possible interpretations
- Required context is missing
- The user might be solving the wrong problem

If any apply, use ask_followup_question with 2-4 suggested answers.

## When to Use This Skill

- Requirements are vague or unclear
- Multiple valid approaches exist
- Missing critical information
- Ambiguous terminology used
- Edge cases not specified

## Process

1. **Identify the ambiguity** in the request
2. **Formulate specific questions** to clarify
3. **Provide 2-4 concrete options** with different implications
4. **Use ask_followup_question** tool to get user input
5. **Proceed only after clarification** is received

## Examples of Good Clarifications

Instead of: "What should I do?"
Ask: "Would you like me to [option 1], [option 2], or [option 3]?"

Instead of: "Fix this bug"
Ask: "The bug could be related to [cause A], [cause B], or [cause C]. Which scenario matches your situation?"
