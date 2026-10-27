// Actual pinned draft-2020-12 schema, with closed-schema negatives.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..');const Ajv=require(path.join(root,'.qa/renderer/node_modules/ajv/dist/2020')).default;
const ajv=new Ajv({strict:false,allErrors:true});ajv.addFormat('uri',value=>URL.canParse(value));const validate=ajv.compile(JSON.parse(fs.readFileSync(path.join(root,'qa/fixtures/host/schemas/extension-v3.schema.json'))));
const manifest=JSON.parse(fs.readFileSync(path.join(root,'extension.json')));assert.ok(validate(manifest),JSON.stringify(validate.errors));let negatives=0;
for(const mutate of [m=>m.schemaVersion=2,m=>m.execution='embedded',m=>m.protocolVersion=2,m=>m.id='raycast.image-base64',m=>m.runtime='javascript',m=>m.repository="not a URI",m=>m.unexpected=true,m=>m.commands[0].platforms=['unsupported'],m=>m.commands[0].concurrency='parallel',m=>m.commands[0].timeoutMs=300001,m=>m.commands[0].refreshMs=1,m=>m.replaces=[],m=>m.replaces.push(m.replaces[0]),m=>m.artifacts={'aarch64-apple-darwin':{url:'file:///tmp/fake',sha256:'broken'}}]){let m=structuredClone(manifest);mutate(m);assert.equal(validate(m),false,JSON.stringify(m));negatives++;}
console.log(JSON.stringify({pinnedSchemaValid:true,negativeCases:negatives,sourceArtifacts:manifest.artifacts,platformAcceptance:'not-run'}));
