import { expect, test } from '@playwright/test';

/**
 * Mock Tauri IPC cho tat ca cac command lien quan den embedding settings.
 * Return data thuc te de test duoc UX flow day du.
 */
function createMockIPC(overrides: Record<string, unknown> = {}) {
  return {
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      console.log(`[MockIPC] ${cmd}`, args);

      const mocks: Record<string, unknown> = {
        check_setup_complete: true,
        get_config: {
          setup_complete: true,
          vault_path: '/tmp/vault',
          remote_name: 'echovault',
          folder_name: 'EchoVault',
        },
        get_app_info: {
          version: '1.18.0',
          data_dir: '/tmp/echovault',
          config_dir: '/tmp/echovault/config',
          logs_dir: '/tmp/echovault/logs',
        },
        get_autostart_status: false,
        scan_sessions: {
          sessions: [
            {
              id: 'test-session-1',
              source: 'VS Code Copilot',
              title: 'Test Session',
              workspace_name: 'TestProject',
              created_at: new Date().toISOString(),
              file_size: 2048,
              path: '/tmp/session-1.json',
            },
          ],
          total: 1,
        },
        sync_vault: 'Sync complete',
        embedding_stats: null,
        get_embedding_config: {
          preset: 'ollama',
          api_base: 'http://localhost:11434/v1',
          api_key: null,
          model: 'nomic-embed-text',
        },
        check_ollama: {
          available: true,
          models: ['nomic-embed-text:latest', 'mxbai-embed-large:latest', 'all-minilm:latest'],
        },
        save_embedding_config: null,
        test_embedding_connection: {
          status: 'available',
          dimension: 768,
          message: null,
        },
        ...overrides,
      };

      if (cmd in mocks) {
        return typeof mocks[cmd] === 'function'
          ? (mocks[cmd] as (...a: unknown[]) => unknown)(args)
          : mocks[cmd];
      }

      return null;
    },
    metadata: {},
  };
}

test.describe('Embedding Settings UX', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      // Placeholder - will be overridden per test via evaluate
    });
  });

  test('Settings overlay shows embedding provider section', async ({ page }) => {
    await page.addInitScript(`
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [], total: 0 },
            sync_vault: 'ok',
            embedding_stats: null,
            get_embedding_config: { preset: 'ollama', api_base: 'http://localhost:11434/v1', api_key: null, model: 'nomic-embed-text' },
            check_ollama: { available: true, models: ['nomic-embed-text:latest', 'mxbai-embed-large:latest'] },
            save_embedding_config: null,
            test_embedding_connection: { status: 'available', dimension: 768, message: null },
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);

    // Open settings
    await page.getByTitle('Settings').click();

    // Verify embedding provider section
    await expect(page.getByText('Embedding Provider')).toBeVisible();

    // Verify preset buttons
    await expect(page.getByRole('button', { name: 'ollama' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'openai' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'custom' })).toBeVisible();

    // Verify Ollama detected message
    await expect(page.getByText('Ollama detected')).toBeVisible();
  });

  test('Ollama preset shows model dropdown when models available', async ({ page }) => {
    await page.addInitScript(`
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [], total: 0 },
            sync_vault: 'ok',
            embedding_stats: null,
            get_embedding_config: { preset: 'ollama', api_base: 'http://localhost:11434/v1', api_key: null, model: 'nomic-embed-text' },
            check_ollama: { available: true, models: ['nomic-embed-text:latest', 'mxbai-embed-large:latest', 'all-minilm:latest'] },
            save_embedding_config: null,
            test_embedding_connection: { status: 'available', dimension: 768, message: null },
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);
    await page.getByTitle('Settings').click();

    // Should show select dropdown (not text input) for model
    const modelSelect = page.locator('select');
    await expect(modelSelect).toBeVisible();

    // Verify all models are in dropdown
    const options = modelSelect.locator('option');
    await expect(options).toHaveCount(3);
    await expect(options.nth(0)).toHaveText('nomic-embed-text:latest');
    await expect(options.nth(1)).toHaveText('mxbai-embed-large:latest');
    await expect(options.nth(2)).toHaveText('all-minilm:latest');
  });

  test('Switching to OpenAI preset shows API key field and text input for model', async ({
    page,
  }) => {
    await page.addInitScript(`
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [], total: 0 },
            sync_vault: 'ok',
            embedding_stats: null,
            get_embedding_config: { preset: 'ollama', api_base: 'http://localhost:11434/v1', api_key: null, model: 'nomic-embed-text' },
            check_ollama: { available: true, models: ['nomic-embed-text:latest'] },
            save_embedding_config: null,
            test_embedding_connection: { status: 'available', dimension: 1536, message: null },
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);
    await page.getByTitle('Settings').click();

    // Switch to OpenAI preset
    await page.getByRole('button', { name: 'openai' }).click();

    // API key field should appear
    await expect(page.getByText('API Key (required)')).toBeVisible();
    await expect(page.getByPlaceholder('sk-...')).toBeVisible();

    // Model should be text input (not select dropdown)
    const modelInputs = page.locator('input[type="text"]');
    // Find the model input by checking its value
    const modelInput = modelInputs.filter({ hasText: '' }).last();
    // The API base should update to OpenAI
    const apiBaseInput = page.locator('input[type="text"]').first();
    await expect(apiBaseInput).toHaveValue('https://api.openai.com/v1');

    // Save & Test button should appear (config is dirty after preset change)
    await expect(page.getByRole('button', { name: /Save & Test/ })).toBeVisible();
  });

  test('Save & Test button triggers save then auto-test', async ({ page }) => {
    let saveCalled = false;
    let testCalled = false;

    await page.addInitScript(`
      window.__test_calls__ = [];
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          window.__test_calls__.push(cmd);
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [], total: 0 },
            sync_vault: 'ok',
            embedding_stats: null,
            get_embedding_config: { preset: 'ollama', api_base: 'http://localhost:11434/v1', api_key: null, model: 'nomic-embed-text' },
            check_ollama: { available: true, models: ['nomic-embed-text:latest'] },
            save_embedding_config: null,
            test_embedding_connection: { status: 'available', dimension: 768, message: null },
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);
    await page.getByTitle('Settings').click();

    // Change model to trigger dirty state
    await page.getByRole('button', { name: 'custom' }).click();

    // Click Save & Test
    await page.getByRole('button', { name: /Save & Test/ }).click();

    // Wait for the save + test to complete
    await page.waitForTimeout(500);

    // Verify save_embedding_config was called before test_embedding_connection
    const calls = await page.evaluate(() => (window as any).__test_calls__);
    const saveIdx = calls.indexOf('save_embedding_config');
    const testIdx = calls.indexOf('test_embedding_connection');
    expect(saveIdx).toBeGreaterThanOrEqual(0);
    expect(testIdx).toBeGreaterThan(saveIdx);

    // Status indicator should show connected
    await expect(page.getByText(/Connected \(dim=768\)/).first()).toBeVisible();
  });

  test('Ollama not running shows warning', async ({ page }) => {
    await page.addInitScript(`
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [], total: 0 },
            sync_vault: 'ok',
            embedding_stats: null,
            get_embedding_config: { preset: 'ollama', api_base: 'http://localhost:11434/v1', api_key: null, model: 'nomic-embed-text' },
            check_ollama: { available: false, models: [] },
            save_embedding_config: null,
            test_embedding_connection: { status: 'unavailable', dimension: null, message: 'Connection refused' },
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);
    await page.getByTitle('Settings').click();

    // Should show "Ollama not running" warning
    await expect(page.getByText('Ollama not running')).toBeVisible();

    // Should show text input (not dropdown) since no models available
    const selectElements = page.locator('select');
    await expect(selectElements).toHaveCount(0);
  });
});

test.describe('Search Tab - Empty State', () => {
  test('Shows 3-step guide when no embeddings exist', async ({ page }) => {
    await page.addInitScript(`
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [{ id: 's1', source: 'Cursor', title: 'Test', workspace_name: null, created_at: new Date().toISOString(), file_size: 1024, path: '/tmp/s1.json' }], total: 1 },
            sync_vault: 'ok',
            embedding_stats: null,
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);

    // Switch to Search tab
    await page.getByRole('button', { name: 'Search' }).click();

    // Verify 3-step guide
    await expect(page.getByText('Get started with semantic search')).toBeVisible();
    await expect(page.getByText('Configure Embedding', { exact: true })).toBeVisible();
    await expect(page.getByText('Test Connection', { exact: true })).toBeVisible();
    await expect(page.getByText('Build Index', { exact: true }).first()).toBeVisible();

    // Verify Build Index guidance text
    await expect(
      page.getByText('Configure embedding provider in Settings, then click Build Index.')
    ).toBeVisible();
  });

  test('Shows search prompt when embeddings exist', async ({ page }) => {
    await page.addInitScript(`
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [], total: 0 },
            sync_vault: 'ok',
            embedding_stats: { total_chunks: 150, total_sessions: 5 },
          };
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);

    // Switch to Search tab
    await page.getByRole('button', { name: 'Search' }).click();

    // Should show search prompt with stats
    await expect(page.getByText('Search your AI conversation history')).toBeVisible();
    await expect(page.getByText(/5 sessions/).first()).toBeVisible();

    // Should NOT show 3-step guide
    await expect(page.getByText('Get started with semantic search')).not.toBeVisible();
  });

  test('Build Index shows embed stats after embedding', async ({ page }) => {
    let embedCalled = false;

    await page.addInitScript(`
      let hasEmbedded = false;
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          const mocks = {
            check_setup_complete: true,
            get_config: { setup_complete: true, vault_path: '/tmp/vault', remote_name: 'echovault', folder_name: 'EchoVault' },
            get_app_info: { version: '1.18.0', data_dir: '/tmp', config_dir: '/tmp', logs_dir: '/tmp' },
            get_autostart_status: false,
            scan_sessions: { sessions: [{ id: 's1', source: 'Cursor', title: 'Test', workspace_name: null, created_at: new Date().toISOString(), file_size: 1024, path: '/tmp/s1.json' }], total: 1 },
            sync_vault: 'ok',
          };

          if (cmd === 'embedding_stats') {
            return hasEmbedded ? { total_chunks: 42, total_sessions: 1 } : null;
          }
          if (cmd === 'embed_sessions') {
            hasEmbedded = true;
            return { sessions_processed: 1, chunks_created: 42, sessions_skipped: 0, errors: 0 };
          }
          return mocks[cmd] ?? null;
        },
        metadata: {}
      };
    `);

    await page.goto('/');
    await page.waitForTimeout(500);

    // Switch to Search tab
    await page.getByRole('button', { name: 'Search' }).click();

    // Click Build Index
    await page.getByRole('button', { name: 'Build Index' }).click();

    // Wait for embedding to complete
    await expect(page.getByText('42 chunks from 1 sessions')).toBeVisible({ timeout: 5000 });
  });
});
