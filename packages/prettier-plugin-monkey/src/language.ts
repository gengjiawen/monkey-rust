import type { SupportLanguage } from 'prettier';

export const languages: SupportLanguage[] = [
  {
    name: 'Monkey',
    parsers: ['monkey'],
    extensions: ['.monkey'],
    vscodeLanguageIds: ['monkey'],
  },
];
