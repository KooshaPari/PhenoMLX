/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{ts,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        'phenotype-teal': '#7ebab5',
        'phenotype-midnight': '#090a0c',
      },
      keyframes: {
        'slide-in-top': {
          '0%': { opacity: '0', transform: 'translateY(-0.5rem)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
      },
      animation: {
        'slide-in-top': 'slide-in-top 0.2s ease-out',
      },
    },
  },
  plugins: [],
}
