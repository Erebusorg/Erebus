# Erebus

Network-layer privacy for tokenized finance on [Robinhood Chain](https://docs.robinhood.com/chain/).

A stock token is an ERC-20, so every position is legible next to an address that
also receives a salary. Shielding transaction *contents* is not enough: the
transport layer still leaks an IP, a timing pattern, and an RPC read history rich
enough to rebuild the portfolio. Erebus is a three-layer Sphinx mixnet with
continuous Poisson mixing, shielded fee payment, and an on-chain node registry, so
no node operator, RPC provider, venue, or global observer can link a user to a
position.

This repository currently holds the site and the specification. The mixnet and
contracts are not implemented yet — see the [roadmap](content/whitepaper.md#9-roadmap).

- **Specification:** [`content/whitepaper.md`](content/whitepaper.md), rendered at `/paper`
- **Status:** Draft 0.1. No mainnet deployment, no audit, no token.

## Development

```bash
npm install
npm run dev     # http://localhost:3000
npm run build
npm run lint
```

## Layout

```
content/whitepaper.md    specification — single source of truth, rendered at /paper
src/app/page.tsx         landing page
src/app/paper/page.tsx   paper renderer
src/components/          nav, footer, reveal-on-scroll, mixnet backdrop, topology, use cases
```
