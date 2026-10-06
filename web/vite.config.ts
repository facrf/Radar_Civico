import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function getVersionInfo() {
	let version = process.env.APP_VERSION && process.env.APP_VERSION.trim() ? process.env.APP_VERSION.trim() : undefined;
	let commit = process.env.APP_COMMIT && process.env.APP_COMMIT.trim() ? process.env.APP_COMMIT.trim() : undefined;
	let count = process.env.APP_COUNT && parseInt(process.env.APP_COUNT, 10) > 0 ? parseInt(process.env.APP_COUNT, 10) : 0;

	// 1. Tentar ler diretamente do Git
	if (!version) {
		try {
			const countStr = execSync('git rev-list --count HEAD', { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
			const cnt = parseInt(countStr, 10);
			if (!isNaN(cnt) && cnt > 0) {
				count = cnt;
				version = `v0.${cnt < 1000 ? String(cnt).padStart(3, '0') : cnt}`;
			}
		} catch {
			// Git indisponível (ex: ambiente Docker sem .git)
		}
	}

	if (!commit) {
		try {
			const hash = execSync('git rev-parse --short HEAD', { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
			if (hash) {
				commit = hash;
			}
		} catch {
			// Git indisponível
		}
	}

	// 2. Fallback seguro: ler do arquivo estático version.json
	if (!version || !commit) {
		const candidateFiles = [
			path.resolve(__dirname, '../version.json'),
			path.resolve(__dirname, 'version.json')
		];
		for (const file of candidateFiles) {
			if (fs.existsSync(file)) {
				try {
					const data = JSON.parse(fs.readFileSync(file, 'utf-8'));
					if (!version && data.version) version = data.version;
					if (!commit && data.commit) commit = data.commit;
					if (!count && data.count) count = data.count;
					break;
				} catch {
					// Fallback silencioso
				}
			}
		}
	}

	return {
		version: version || 'v0.001',
		commit: commit || 'unknown',
		count: count || 1
	};
}

const versionInfo = getVersionInfo();

export default defineConfig({
	plugins: [sveltekit()],
	define: {
		__APP_VERSION__: JSON.stringify(versionInfo.version),
		__APP_COMMIT__: JSON.stringify(versionInfo.commit),
		__APP_COUNT__: JSON.stringify(versionInfo.count)
	},
	server: {
		proxy: {
			'/api': 'http://localhost:8080'
		}
	}
});
