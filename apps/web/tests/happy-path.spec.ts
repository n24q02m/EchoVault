
import { test, expect } from '@playwright/test';

test('Happy Path: Setup Wizard -> Main Dashboard', async ({ page }) => {
    // Mock Tauri IPC
    await page.addInitScript(() => {
        window.__TAURI_INTERNALS__ = {
            invoke: async (cmd, args) => {
                console.log(`[MockIPC] ${cmd}`, args);

                if (cmd === 'check_setup_complete') {
                    return false; // Force Setup Wizard
                }

                if (cmd === 'start_auth') {
                    return { status: 'authenticated', message: null };
                }

                if (cmd === 'complete_setup') {
                    return null;
                }

                if (cmd === 'get_config') {
                    return { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' };
                }

                if (cmd === 'scan_sessions') {
                    return {
                        sessions: [
                            {
                                id: 'session-123',
                                source: 'Cursor',
                                title: 'Refactoring EchoVault',
                                workspace_name: 'AI_Projects',
                                created_at: new Date().toISOString(),
                                file_size: 1024 * 5,
                                path: '/tmp/session-123.json'
                            },
                            {
                                id: 'session-456',
                                source: 'VS Code',
                                title: 'Debugging CI/CD',
                                workspace_name: null,
                                created_at: new Date().toISOString(),
                                file_size: 2048,
                                path: '/tmp/session-456.json'
                            }
                        ],
                        total: 2
                    };
                }

                if (cmd === 'sync_vault') {
                    return "Sync complete";
                }

                return null;
            },
            metadata: {}
        };
    });

    // 1. Load App
    await page.goto('/');

    // 2. Verify Setup Wizard
    await expect(page.getByText('First Time Setup')).toBeVisible();

    // 3. Step 1: Connect
    await page.getByRole('button', { name: 'Connect Cloud Storage' }).click();
    // Should auto-advance to step 2 because mock returns 'authenticated' immediately

    // 4. Step 2: Config
    await expect(page.getByText('2. Configure Sync Folder')).toBeVisible();
    const input = page.getByPlaceholder('EchoVault');
    await expect(input).toHaveValue('EchoVault');

    // 5. Complete Setup
    await page.getByRole('button', { name: 'Complete Setup' }).click();

    // 6. Verify Completion
    await expect(page.getByText('Setup Complete!')).toBeVisible({ timeout: 5000 });

    // 7. Verify Transition to Main Dashboard (View changes to 'main')
    // The 'main' view loads sessions.
    await expect(page.getByText('Refactoring EchoVault')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Cursor')).toBeVisible();
    await expect(page.getByText('VS Code')).toBeVisible();

    // 8. Test Sync Button
    await page.getByRole('button', { name: 'Sync', exact: true }).click();
    await expect(page.getByText('Sync completed successfully')).toBeVisible(); // Toast check
});
