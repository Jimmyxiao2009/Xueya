import {test,expect,vi} from 'vitest';
import {api} from '@/api';
import {useLearningStore} from '@/stores/learningStore';
import {useCloudStore} from '@/stores/cloudStore';

test('astra: late learningStart A cannot overwrite B',async()=>{
 let resolveA!:(v:Awaited<ReturnType<typeof api.learningStart>>)=>void;
 let resolveB!:(v:Awaited<ReturnType<typeof api.learningStart>>)=>void;
 const a=new Promise<Awaited<ReturnType<typeof api.learningStart>>>(r=>{resolveA=r;});
 const b=new Promise<Awaited<ReturnType<typeof api.learningStart>>>(r=>{resolveB=r;});
 const mock=vi.spyOn(api,'learningStart').mockImplementationOnce(()=>a).mockImplementationOnce(()=>b);
 useLearningStore.getState().reset();
 const first=useLearningStore.getState().start('node-A');
 const second=useLearningStore.getState().start('node-B');
 resolveB({sessionId:'session-B',lesson:{title:'B'}} as Awaited<ReturnType<typeof api.learningStart>>);await second;
 resolveA({sessionId:'session-A',lesson:{title:'A'}} as Awaited<ReturnType<typeof api.learningStart>>);await first;
 expect(useLearningStore.getState().nodeId).toBe('node-B');
 expect(useLearningStore.getState().sessionId).toBe('session-B');
 expect(useLearningStore.getState().lesson?.title).toBe('B');
mock.mockRestore();
});

test('astra: usage response from A is discarded after logout',async()=>{
 let resolve!:(v:Awaited<ReturnType<typeof api.cloudGetAiUsage>>)=>void;
 const p=new Promise<Awaited<ReturnType<typeof api.cloudGetAiUsage>>>(r=>{resolve=r;});
 const usage=vi.spyOn(api,'cloudGetAiUsage').mockImplementationOnce(()=>p);
 const logout=vi.spyOn(api,'cloudLogout').mockResolvedValue({state:'signedOut',loggedIn:false} as Awaited<ReturnType<typeof api.cloudLogout>>);
 const pending=useCloudStore.getState().loadUsage();
 await useCloudStore.getState().logout();expect(useCloudStore.getState().usage).toBeNull();
 const old={creditsUsed:123} as Awaited<ReturnType<typeof api.cloudGetAiUsage>>;
 resolve(old);await pending;
 expect(useCloudStore.getState().sessionState).toBe('signedOut');expect(useCloudStore.getState().usage).toBeNull();
usage.mockRestore();logout.mockRestore();
});
