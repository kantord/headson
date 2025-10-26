#!/usr/bin/env node
// Move cluster label elements to the end of the SVG so they render above edges.
// Also ensure they have a white backdrop similar to edge labels.

const fs = require('fs');
const path = process.argv[2];
if (!path) {
  console.error('Usage: node docs/diagrams/postprocess.js <svg-path>');
  process.exit(1);
}

let svg = fs.readFileSync(path, 'utf8');

// Collect cluster label foreignObjects or <g class="cluster-label"> groups
const blocks = [];

function collect(pattern) {
  let m;
  while ((m = pattern.exec(svg)) !== null) {
    blocks.push(m[0]);
  }
  svg = svg.replace(pattern, '');
}

// foreignObject cluster labels (htmlLabels=true)
collect(/<foreignObject[^>]*class="[^"]*cluster-label[^"]*"[\s\S]*?<\/foreignObject>/g);
// group-based cluster labels (htmlLabels=false)
collect(/<g[^>]*class="[^"]*cluster-label[^"]*"[\s\S]*?<\/g>/g);

if (blocks.length > 0) {
  // Append labels right before closing </svg>
  const inject = '\n<!-- moved cluster labels to top layer -->\n' + blocks.join('\n') + '\n';
  svg = svg.replace(/<\/svg>\s*$/i, inject + '</svg>');
  fs.writeFileSync(path, svg, 'utf8');
  console.error(`[postprocess] moved ${blocks.length} cluster label blocks to top layer`);
} else {
  console.error('[postprocess] no cluster labels found to move');
}

