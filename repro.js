const { useState, useEffect } = require('react');

// Mocking the behavior
let renderCount = 0;
let expandedSources = new Set();
let sessions = [];

function setExpandedSources(newSet) {
    // changing state triggers re-render in React
    // In this mock, we just check if it's a new reference or value
    if (expandedSources !== newSet) {
        // Check deep equality for Set (React doesn't do this, it just checks reference)
        // But even if content is same, new Set() is a new reference.
        expandedSources = newSet;
        renderCount++;
        console.log('State updated. Render count:', renderCount);
        console.log('Expanded Sources:', [...expandedSources]);
    }
}

function App() {
    // Current Logic
    if (sessions.length > 0) {
        const sources = [...new Set(sessions.map((s) => s.source).filter(Boolean))];
        setExpandedSources(new Set(sources));
    }
}

// Scenario
console.log('--- Initial Load ---');
sessions = [{ source: 'A' }, { source: 'B' }];
App();

console.log('--- User Collapses A ---');
// User action
expandedSources = new Set(['B']);
console.log('Expanded Sources:', [...expandedSources]);

console.log('--- Data Refresh (Same Data) ---');
// Sync happens, sessions is a new array with same content
sessions = [{ source: 'A' }, { source: 'B' }];
App();

console.log('--- Data Refresh (New Data) ---');
sessions = [{ source: 'A' }, { source: 'B' }, { source: 'C' }];
App();
