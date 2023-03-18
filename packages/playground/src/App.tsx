import React, { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Grid,
  GridItem,
  HStack, Tab, TabList, TabPanel, TabPanels, Tabs,
  Text,
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
  let [editor_value] = useState(code)
  const editorOnchange = (value: string) => {
    console.log(value)
    editor_value = value
    debouncedChangeHandler()
    console.log(`change finished`)
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
      <Editor onChange={editorOnchange} code={editor_value} />
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
