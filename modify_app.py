import sys

filepath = "apps/web/src/App.tsx"
with open(filepath, "r") as f:
    content = f.read()

# Insert useRef initialization
if "const initialized = useRef(false);" not in content:
    content = content.replace(
        "function MainApp() {",
        "function MainApp() {\n  const initialized = useRef(false);"
    )

# Replace useEffect
target_block = """  useEffect(() => {
    if (sessions.length > 0) {
      const sources = [...new Set(sessions.map((s) => s.source).filter(Boolean))] as string[];
      setExpandedSources(new Set(sources));
    }
  }, [sessions]);"""

replacement_block = """  useEffect(() => {
    if (sessions.length > 0 && !initialized.current) {
      const sources = [...new Set(sessions.map((s) => s.source).filter(Boolean))] as string[];
      setExpandedSources(new Set(sources));
      initialized.current = true;
    }
  }, [sessions]);"""

if target_block in content:
    content = content.replace(target_block, replacement_block)
    print("Successfully modified useEffect.")
else:
    print("Target block not found!")
    # Debug: print surrounding lines if possible or check for minor whitespace differences
    # Let's try to match with less whitespace strictness if needed, but file seems formatted.
    pass

with open(filepath, "w") as f:
    f.write(content)
