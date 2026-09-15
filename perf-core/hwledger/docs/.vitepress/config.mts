// hwLedger VitePress config.
// Served at kooshapari.github.io/hwLedger/ — base='/hwLedger/' on GitHub Pages.
// No custom domain configured. If one is added later: set PHENOTYPE_CUSTOM_DOMAIN=true
// in the deploy workflow and add a CNAME file under docs/public/.
import { createPhenotypeConfig } from '@phenotype/docs/config'

const isPagesBuild =
  process.env.GITHUB_ACTIONS === 'true' || process.env.GITHUB_PAGES === 'true'
const repoName = process.env.GITHUB_REPOSITORY?.split('/')[1] ?? 'hwLedger'
// Honor custom-domain override: PHENOTYPE_CUSTOM_DOMAIN=true → serve from /
const customDomain = process.env.PHENOTYPE_CUSTOM_DOMAIN === 'true'
const docsBase = customDomain ? '/' : isPagesBuild ? `/${repoName}/` : '/'

export default createPhenotypeConfig({
  title: 'hwLedger',
  description: 'Phenotype-org hardware ledger',
  srcDir: '.',
  base: docsBase,
  githubOrg: 'KooshaPari',
  githubRepo: repoName,

  nav: [
    { text: 'ADR', link: '/adr/0001-record-architecture-decisions' },
    { text: 'Journeys', link: '/journeys/journey-traceability' },
    { text: 'Operations', link: '/operations/' },
  ],

  sidebar: {
    '/adr/': [
      {
        text: 'Architecture Decision Records',
        items: [
          {
            text: 'Record Architecture Decisions',
            link: '/adr/0001-record-architecture-decisions',
          },
        ],
      },
    ],
    '/journeys/': [
      {
        text: 'Journeys',
        items: [
          {
            text: 'Journey Traceability',
            link: '/journeys/journey-traceability',
          },
        ],
      },
    ],
  },
})
