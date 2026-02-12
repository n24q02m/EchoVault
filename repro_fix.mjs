// Mocking
let renderCount = 0;
let expandedSources = new Set();
let sessions = [];

// Mocking useRef
let initialized = { current: false };

function setExpandedSources(newSet) {
    // changing state triggers re-render
    expandedSources = newSet;
    renderCount++;
    console.log('State updated. Render count:', renderCount);
    console.log('Expanded Sources:', [...expandedSources]);
}

function AppEffect() {
    // Proposed Fix
    if (sessions.length > 0 && !initialized.current) {
        const sources = [...new Set(sessions.map((s) => s.source).filter(Boolean))];
        setExpandedSources(new Set(sources));
        initialized.current = true;
    }
}

// Scenario
console.log('--- Initial Load ---');
sessions = [{ source: 'A' }, { source: 'B' }];
AppEffect();

console.log('--- User Collapses A ---');
// User action
expandedSources = new Set(['B']);
console.log('Expanded Sources (After Collapse):', [...expandedSources]);

console.log('--- Data Refresh (Same Data) ---');
sessions = [{ source: 'A' }, { source: 'B' }];
AppEffect();
// Expected: NO change to expandedSources. User preference preserved.

console.log('--- Data Refresh (New Data) ---');
sessions = [{ source: 'A' }, { source: 'B' }, { source: 'C' }];
AppEffect();
// Expected: NO change (C is collapsed by default).
// If this is acceptable, then the fix is good.
// The user might want "Auto-expand new sources", but fixing the "Reset" issue is primary.
// If "C" is not in expandedSources, it's just collapsed. The user can expand it.
