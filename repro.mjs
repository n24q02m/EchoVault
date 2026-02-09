// Mocking the behavior
let renderCount = 0;
let expandedSources = new Set();
let sessions = [];

function setExpandedSources(newSet) {
    // In React, setting state to a NEW reference triggers re-render
    // Even if content is identical.
    expandedSources = newSet;
    renderCount++;
    console.log('State updated. Render count:', renderCount);
    console.log('Expanded Sources:', [...expandedSources]);
}

function AppEffect() {
    // Current Logic from App.tsx
    if (sessions.length > 0) {
        const sources = [...new Set(sessions.map((s) => s.source).filter(Boolean))];
        setExpandedSources(new Set(sources));
    }
}

// Scenario
console.log('--- Initial Load ---');
sessions = [{ source: 'A' }, { source: 'B' }];
AppEffect();

console.log('--- User Collapses A ---');
// User action simulation
expandedSources = new Set(['B']);
console.log('Expanded Sources (After Collapse):', [...expandedSources]);

console.log('--- Data Refresh (Same Data) ---');
// Sync happens, sessions is a new array with same content
sessions = [{ source: 'A' }, { source: 'B' }];
// Effect runs because sessions dependency changes (new array reference)
AppEffect();
// Expected: expandedSources resets to include A again

console.log('--- Data Refresh (New Data) ---');
sessions = [{ source: 'A' }, { source: 'B' }, { source: 'C' }];
AppEffect();
