import { SettingRow } from "./shared";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

export function SettingsView() {
  return (
    <Tabs defaultValue="runtime">
      <TabsList>
        <TabsTrigger value="runtime">Runtime</TabsTrigger>
        <TabsTrigger value="headers">Headers</TabsTrigger>
      </TabsList>
      <TabsContent value="runtime">
        <Card>
          <CardHeader>
            <CardTitle>Runtime</CardTitle>
            <CardDescription>Current service assumptions exposed by the frontend.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4 md:grid-cols-2">
            <SettingRow label="Anthropic Messages" value="/anthropic/v1/messages" />
            <SettingRow label="Codex Responses" value="/openai/v1/responses" />
            <SettingRow label="OpenAI Chat" value="/openai/v1/chat/completions" />
            <SettingRow
              label="Gemini GenerateContent"
              value="/gemini/v1beta/models/{model}:generateContent"
            />
            <SettingRow label="Admin API" value="/admin/api" />
            <SettingRow label="Storage" value="SQLite" />
          </CardContent>
        </Card>
      </TabsContent>
      <TabsContent value="headers">
        <Card>
          <CardHeader>
            <CardTitle>Forwarded headers</CardTitle>
            <CardDescription>Headers preserved or supplied by the Rust relay.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {[
              "x-api-key",
              "x-goog-api-key",
              "authorization: Bearer",
              "anthropic-version",
              "anthropic-beta",
            ].map((item) => (
              <div key={item} className="rounded-md border p-3 font-mono text-sm">
                {item}
              </div>
            ))}
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>
  );
}
