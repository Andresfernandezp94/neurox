# Incident Postmortem: [REQUIRED: short title of the incident]

> **Blameless postmortem**. Based on Google SRE Chapter 15.
> Foco en qué pasó, no en quién tuvo la culpa.

## Metadata

- Incident ID: [REQUIRED: INC-YYYY-NNN]
- Date of incident: [REQUIRED: YYYY-MM-DD]
- Date of postmortem: [REQUIRED: YYYY-MM-DD]
- Severity: [REQUIRED: Critical|High|Medium|Low]
- Mode: incident
- Author: [REQUIRED: agent or human who wrote this postmortem]
- Reviewers: [REQUIRED: list of people who reviewed]
- Status: [REQUIRED: Draft|Reviewed|Published]

## Summary

[REQUIRED: 1 paragraph executive summary. What happened, what was the impact, what was the root cause, what changed.]

## Timeline

[REQUIRED: chronological list of events. Each entry: timestamp (UTC) + actor + event]

| Timestamp (UTC) | Actor | Event |
|---|---|---|
| [REQUIRED] | [REQUIRED] | [REQUIRED] |
| [REQUIRED] | [REQUIRED] | [REQUIRED] |
| [REQUIRED] | [REQUIRED] | [REQUIRED] |

## Impact

[REQUIRED: who/what was affected, for how long, with what consequences]

- **Users affected**: [N]
- **Duration**: [X hours]
- **Data exposed**: [yes/no + what]
- **Financial cost**: [estimate, optional]
- **Reputational impact**: [description, optional]

## Root Cause

[REQUIRED: the actual cause, not just the symptom. Use 5-whys if needed.]

### Why 1: [symptom observed]?
Because [first-level cause].

### Why 2: [first-level cause]?
Because [second-level cause].

### Why 3: [second-level cause]?
Because [root cause].

[Continue until you reach systemic cause]

### Root cause
[REQUIRED: 1-2 sentences describing the systemic cause that, if addressed, would have prevented this incident]

## Contributing Factors

[REQUIRED: things that made the incident worse or harder to detect. Not the root cause, but enabled it.]

- [Factor 1: e.g., "logs were not centralized"]
- [Factor 2: e.g., "no automated alerting on this metric"]

## What Went Well

[REQUIRED: also blameless — recognize what worked]

- Detection happened within [X minutes] due to [alert/user report]
- Response was coordinated by [name]
- Communication was clear throughout

## What Went Wrong

[REQUIRED: blameless — what should have been better]

- Detection took [X minutes] longer than it should have
- Initial response was delayed because [reason]
- Documentation was missing for [area]

## Detection

[REQUIRED: how was the incident detected?]

- **Detection method**: [alerting / user report / monitoring / etc.]
- **Detection time**: [X minutes after start]
- **Detection should have been**: [X minutes — what's the ideal?]

## Response

[REQUIRED: chronological summary of what was done to mitigate. Link to detailed timeline above.]

## Lessons Learned

[REQUIRED: what did we learn that we didn't know before?]

1. [Lesson 1: "We learned that..."]
2. [Lesson 2: "We learned that..."]

## Action Items

[REQUIRED: concrete actions to prevent recurrence. Each has owner + deadline + priority.]

| # | Action | Priority | Owner | Deadline | Status |
|---|---|---|---|---|---|
| 1 | [REQUIRED] | [REQUIRED: Critical\|High\|Medium\|Low] | [REQUIRED] | [REQUIRED: YYYY-MM-DD] | [REQUIRED: Open\|InProgress\|Done] |
| 2 | [REQUIRED] | [...] | [...] | [...] | [...] |

## Related

[OPTIONAL: links to related incidents, findings, ADRs, docs]

- Related findings: [SEC-XXXX-YYYY-NNN]
- Related ADRs: [ADR-NNNN]
- Related proposals: [EP-NNNN]
- External references: [URLs]

## Validation

Before publishing:

- [ ] Timeline is accurate (cross-check with logs)
- [ ] Root cause is systemic (not blaming individuals)
- [ ] Action items have owners and deadlines
- [ ] All reviewers have signed off
- [ ] `bash security/bin/validate-incident.sh <this-file>.md` exits 0

## References

- Google SRE Book, Chapter 15: https://sre.google/sre-book/postmortem-culture/
- Etsy Morgue: https://github.com/etsy/morgue
- security/templates/finding.template.md — for action items that are findings
- security/process.md — overall process
