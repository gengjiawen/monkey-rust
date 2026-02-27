import React, { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Button,
  Grid,
  GridItem,
  HStack, Tab, TabList, TabPanel, TabPanels, Tabs,
  Text,
  VStack,
} from '@chakra-ui/react'

import { Editor } from './Editor'
import debounce from 'lodash.debounce'
import {compile} from "@gengjiawen/monkey-wasm";

const big_sample = `
1 + 1;
if (true) { 10 }; 3333;
let a = [1, 2, 3];
`
const sample_list = [
    '1 + 1',
    'if (true) { 10 }; 3333;',
    big_sample,
]

function App() {
  let code = big_sample.trimStart()
  let [editor_value, setEditorValue] = useState(code)
  const editorOnchange = (value: string) => {
    console.log(value)
    editor_value = value
    setEditorValue(value)
    debouncedChangeHandler()
    console.log(`change finished`)
  }

  const formatCode = async () => {
    try {
      const prettier = await import('prettier/standalone')
      const monkeyPlugin = await import('prettier-plugin-monkey')
      const formatted = await prettier.format(editor_value, {
        parser: 'monkey',
        plugins: [monkeyPlugin],
      })
      setEditorValue(formatted)
      editor_value = formatted
      getRes()
    } catch (e: any) {
      console.error('Format error:', e)
    }
  }

  let [compiler_out, setCompilerout] = useState('')
  const getRes = () => {
    try {
      const bytecode = compile(editor_value)
      setCompilerout(bytecode)
    } catch(e: any) {
      setCompilerout(e.toString())
    }
  }

  const debouncedChangeHandler = useMemo(() => debounce(getRes, 200), [getRes])

  useEffect(() => {
    getRes()
  }, [])

  return (
    <Grid templateColumns="repeat(2, 1fr)" height="100vh" gap={6}>
      <GridItem display="flex" flexDirection="column">
        <HStack p={2}>
          <Button size="sm" onClick={formatCode}>Format</Button>
        </HStack>
        <Editor onChange={editorOnchange} code={editor_value} />
      </GridItem>
      <Tabs size='md' variant='enclosed'>
        <TabList>
          <Tab>Bytecode</Tab>
        </TabList>
        <TabPanels>
          <TabPanel padding={0}>
              <Editor
                  code={compiler_out}
                  extra={{ readOnly: true, editable: false }}
              />
          </TabPanel>
          <TabPanel>
            <p>two!</p>
          </TabPanel>
        </TabPanels>
      </Tabs>
    </Grid>
  )
}

export default App
