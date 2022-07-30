import React, { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Grid,
  GridItem,
  HStack,
  Text,
} from '@chakra-ui/react'
import { Editor } from './Editor'
import debounce from 'lodash.debounce'
import {compile} from "@gengjiawen/monkey-wasm";

const sample_list = [
    '1 + 1',
    'if (true) { 10 }; 3333;',
]

function App() {
  let code = sample_list[1]
  let [editor_value] = useState(code)
  const editorOnchange = (value: string) => {
    console.log(value)
    editor_value = value
    debouncedChangeHandler()
    console.log(`change finished`)
  }

  let [compiler_out, setCompilerout] = useState('')
  const getRes = () => {
    const bytecode = compile(editor_value)
    setCompilerout(bytecode)
  }

  const debouncedChangeHandler = useMemo(() => debounce(getRes, 500), [getRes])

  useEffect(() => {
    getRes()
  }, [])

  return (
    <Grid templateColumns="repeat(2, 1fr)" height="100vh" gap={6}>
      <Editor onChange={editorOnchange} code={editor_value} />
      <GridItem w="100%" h="50%">
        <HStack>
          <Text>Bytecode:</Text>
        </HStack>
        <Editor
          code={compiler_out}
          extra={{ readOnly: true, editable: false }}
        />
      </GridItem>
    </Grid>
  )
}

export default App
