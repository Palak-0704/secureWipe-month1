

# Frontend Research & Progress (Month 1)

## My Research Journey (Student Perspective)

When I started working on the SecureWipe frontend, I honestly didn’t know which framework would be best. I spent a lot of time reading about different options like React, Angular, and Vue. After talking to friends and checking out some tutorials, I picked **React** because it’s popular, beginner-friendly, and there are tons of resources online. I also learned about **Vite**, which makes React projects run and reload super fast, so I used that for the setup.

For styling, I wanted to avoid messy CSS, so I used **CSS Modules**. This way, each component has its own styles, and nothing gets mixed up. I also added **Google Fonts** and **Material Icons** because I wanted the app to look modern and clean, just like the apps I use every day.

## How I Designed the UI

I started by sketching some wireframes on paper and then tried to match them in code. The main screens I focused on were:
- Welcome screen
- Dashboard
- Device Selection
- Advisor screen

I made sure the layout uses the whole screen (edge-to-edge) because it looks more professional. I kept the navigation simple, with clear buttons and sections, so anyone can use it without getting lost. I also made sure the fonts and icons are the same everywhere for a consistent look.

## Making Components Modular

One thing I learned is that breaking the UI into small pieces (components) makes everything easier. So, I created separate files for things like the WelcomeScreen, Sidebar, Header, etc. This way, if I need to change something later, I only have to update one file. It also helps if someone else wants to work on the project with me.

## Adding Multiple Languages

I wanted the app to be useful for more people, so I added support for English and Hindi. All the text is stored in the `locales/` folder, so it’s easy to add more languages in the future. The app automatically shows the right language based on what the user picks.

## Handling Errors and Feedback

I made sure that whenever something important happens (like a success or error), the app shows a clear message. Before doing anything risky, like wiping a device, there’s a confirmation dialog. For long tasks, there are progress bars so users know what’s happening.

## Security Stuff I Learned

I read about frontend security and learned not to put any secret info (like API keys) in the code. All the important work (like wiping devices) is done by the backend, and the frontend just sends safe requests. I also made sure to check and clean any user input before sending it to the backend.

## Themes and Customization

I added light and dark mode because I like having the choice, and I know a lot of people do too. The app remembers your preferences, like language and theme, so you don’t have to set them every time.

## Connecting to the Backend

The frontend talks to the backend using secure APIs. All the device stuff and wiping is handled by the backend, so the frontend just shows the results and progress.

## What I Did, Step by Step
1. Researched different frontend frameworks and picked React with Vite.
2. Sketched wireframes and planned how users would move through the app.
3. Set up the project and made all the main screens and navigation.
4. Broke everything into components and added styles with CSS Modules.
5. Added support for multiple languages and themes.
6. Tested everything and fixed bugs as I found them.
7. Wrote this summary to explain my work and research.

## References and Links I Used
- [React Documentation](https://react.dev/)
- [Vite Documentation](https://vitejs.dev/)
- [Google Fonts](https://fonts.google.com/)
- [Material Icons Guide](https://fonts.google.com/icons)
- [Material Design Guidelines](https://m3.material.io/)
- [MDN Web Docs: Localization](https://developer.mozilla.org/en-US/docs/Web/Localization)
- [OWASP Desktop App Security](https://owasp.org/www-project-desktop-app-security/)

## Final Thoughts

This was my first time building a real frontend for a project, and I learned a lot! I tried to follow best practices but also made sure to keep things simple and clear. If you have any feedback or ideas, I’d love to hear them.