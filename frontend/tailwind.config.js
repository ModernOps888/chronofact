/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"Plus Jakarta Sans"', 'system-ui', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'monospace'],
      },
      colors: {
        obsidian: {
          950: '#06080d',
          900: '#0a0e17',
          850: '#0f1422',
          800: '#141b2d',
          700: '#1e293b',
          600: '#334155',
        },
        gold: {
          50: '#fffdf5',
          100: '#fef9c3',
          200: '#fef08a',
          300: '#fde047',
          400: '#facc15',
          500: '#eab308',
          600: '#ca8a04',
          700: '#a16207',
          800: '#854d0e',
          900: '#713f12',
          DEFAULT: '#fbbf24',
          champagne: '#f7e7ce',
          metallic: '#d4af37',
        },
        emerald: {
          50: '#ecfdf5',
          100: '#d1fae5',
          200: '#a7f3d0',
          300: '#6ee7b7',
          400: '#34d399',
          500: '#10b981',
          600: '#059669',
          700: '#047857',
          800: '#065f46',
          900: '#064e3b',
          950: '#022c22',
        },
      },
      boxShadow: {
        'gold-glow': '0 0 25px rgba(250, 204, 21, 0.2)',
        'gold-glow-lg': '0 0 45px rgba(250, 204, 21, 0.35)',
        'emerald-glow': '0 0 25px rgba(16, 185, 129, 0.2)',
      },
      backgroundImage: {
        'gold-gradient': 'linear-gradient(135deg, #fef08a 0%, #facc15 50%, #ca8a04 100%)',
        'gold-shimmer': 'linear-gradient(90deg, #ca8a04 0%, #fef08a 50%, #ca8a04 100%)',
        'dark-mesh': 'radial-gradient(at 0% 0%, rgba(250, 204, 21, 0.08) 0px, transparent 50%), radial-gradient(at 100% 100%, rgba(16, 185, 129, 0.08) 0px, transparent 50%)',
      },
    },
  },
  plugins: [],
}
