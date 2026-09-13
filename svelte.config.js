import adapter from '@sveltejs/adapter-static'; // Change from adapter-auto to adapter-static
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html', // Essential for Single Page App (SPA) routing inside Tauri
			precompress: false,
			strict: true
		})
	}
};

export default config;
