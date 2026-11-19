# ADR-0006: Placement by searching a concept ladder

- Status: Accepted
- Date: 2026-11-18

## Context

Atlas has to estimate what a candidate already knows about an arbitrary topic in
a few multiple-choice questions. The research read for this decision says:

- Adaptive tests are short and precise only with a bank of questions calibrated
  on real test-takers
  ([survey](https://arxiv.org/html/2404.00712v4),
  [Ebenbeck 2024](https://onlinelibrary.wiley.com/doi/full/10.1111/jcal.13074)).
  Atlas writes its questions on the spot, so a numeric ability score would be
  false precision.
- ALEKS places learners by walking a prerequisite structure, where an answer on
  one concept is evidence about its neighbours
  ([reliability study](https://link.springer.com/article/10.1007/s40593-019-00176-0)).
- Self-rated knowledge tracks motivation more than tested knowledge
  ([Sitzmann et al.](https://journals.aom.org/doi/10.5465/amle.9.2.zqr169)).
- A right multiple-choice answer mixes knowledge with guessing, and a confident
  wrong one signals a misconception rather than a gap
  ([study](https://pmc.ncbi.nlm.nih.gov/articles/PMC10201919/)).
- Asking for the first step of a solution measures expertise quickly
  ([Kalyuga and Sweller](https://link.springer.com/article/10.1007/BF02504800)).
- Model-written questions carry wrong answer keys at a rate that varies widely
  by model
  ([study](https://www.frontiersin.org/journals/medicine/articles/10.3389/fmed.2026.1978708/full)),
  and tutors given a level drift from it over long sessions
  ([synthesis](https://arxiv.org/html/2610.15692)).

## Decision

1. The candidate gives one self-rating and the goal (overview, working
   knowledge, expert).
2. The agent drafts, unseen, a ladder of eight to twelve concepts in
   prerequisite order.
3. Four to six questions follow, one at a time, starting at the self-rated rung,
   moving up after a right answer and down after a wrong one or "Not sure". It
   stops once two neighbouring rungs disagree.
4. Every question has "Not sure". Wrong options are written as specific
   misconceptions. Questions ask what to do or what happens next, not for
   definitions.

The result is the candidate profile: each rung marked known, shaky or unknown,
plus the misconceptions picked. There is no single score. The index is built
from it.

Placement is a starting estimate. Check questions keep correcting the profile,
and it is sent with every generation.

## Tradeoff

One hidden request drafts the ladder before the first question appears. Answers
carry no confidence rating, so a lucky guess counts as knowledge until a check
question says otherwise. Placement cannot be skipped.
